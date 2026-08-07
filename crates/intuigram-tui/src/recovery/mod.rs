use std::path::PathBuf;

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::source::{BACKGROUND, CHROME_BACKGROUND, MUTED_TEXT, PRIMARY, SURFACE_BACKGROUND, TEXT};

/// Read-only information shown when an Account database cannot be trusted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryView {
    /// Account label from the global registry.
    pub account_name: String,

    /// Exact database path that failed validation.
    pub database_path: PathBuf,

    /// Existing pre-migration and recovery backup paths.
    pub backup_paths: Vec<PathBuf>,

    /// Typed storage failure rendered for diagnosis.
    pub failure: String,

    /// Reason a safe rebuild cannot be offered, when unique data is unreadable.
    pub rebuild_blocker: Option<String>,

    /// Completion notice after a successful rebuild.
    pub completion: Option<String>,

    /// Non-terminal feedback from an attempted recovery action.
    pub notice: Option<String>,
}

/// Explicit action selected from the Account recovery screen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryAction {
    /// Retry migrations and consistency checks without changing the database.
    Retry,

    /// Reveal the newest backup, or the failed database when none exists.
    OpenBackupLocation,

    /// Rebuild only synchronized cache after unique-record verification.
    RebuildCache,

    /// Leave Intuigram without modifying Account data.
    Cancel,

    /// Redraw after a terminal resize.
    Redraw,

    /// Continue into the rebuilt Account after reviewing its backup path.
    Continue,
}

/// Resolves one raw event against the recovery screen's visible actions.
#[must_use]
pub fn resolve_recovery_event(event: Event, can_rebuild: bool) -> Option<RecoveryAction> {
    match event {
        Event::Resize(..) => Some(RecoveryAction::Redraw),
        Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
            match key.code {
                KeyCode::Char('r' | 'R') if key.modifiers == KeyModifiers::NONE => {
                    Some(RecoveryAction::Retry)
                }
                KeyCode::Char('o' | 'O') if key.modifiers == KeyModifiers::NONE => {
                    Some(RecoveryAction::OpenBackupLocation)
                }
                KeyCode::Char('b' | 'B') if can_rebuild && key.modifiers == KeyModifiers::NONE => {
                    Some(RecoveryAction::RebuildCache)
                }
                KeyCode::Esc | KeyCode::Char('q' | 'Q') if key.modifiers == KeyModifiers::NONE => {
                    Some(RecoveryAction::Cancel)
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    Some(RecoveryAction::Cancel)
                }
                KeyCode::Enter => Some(RecoveryAction::Continue),
                _ => None,
            }
        }
        _ => None,
    }
}

pub(super) fn render(frame: &mut Frame<'_>, view: &RecoveryView) {
    let area = frame.area();
    frame.render_widget(
        Paragraph::new("").style(Style::default().bg(BACKGROUND)),
        area,
    );
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(8),
        Constraint::Length(2),
        Constraint::Length(1),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "Account database recovery",
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                format!("{} is opened read-only", view.account_name),
                Style::default().fg(MUTED_TEXT),
            )),
        ])
        .style(Style::default().bg(SURFACE_BACKGROUND)),
        rows[0],
    );

    let mut details = vec![
        Line::from(Span::styled(
            "Database",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(view.database_path.display().to_string()),
        Line::from(""),
        Line::from(Span::styled(
            "Failure",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(view.failure.clone()),
        Line::from(""),
        Line::from(Span::styled(
            "Backups",
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ];
    if view.backup_paths.is_empty() {
        details.push(Line::from("No previous backup was found"));
    } else {
        details.extend(
            view.backup_paths
                .iter()
                .map(|path| Line::from(path.display().to_string())),
        );
    }
    if let Some(blocker) = &view.rebuild_blocker {
        details.push(Line::from(""));
        details.push(Line::from(Span::styled(
            "Rebuild unavailable: export or explicit abandonment is required",
            Style::default().fg(MUTED_TEXT).add_modifier(Modifier::BOLD),
        )));
        details.push(Line::from(blocker.clone()));
    }
    if let Some(completion) = &view.completion {
        details.push(Line::from(""));
        details.push(Line::from(Span::styled(
            completion.clone(),
            Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD),
        )));
    }
    if let Some(notice) = &view.notice {
        details.push(Line::from(""));
        details.push(Line::from(Span::styled(
            notice.clone(),
            Style::default().fg(MUTED_TEXT),
        )));
    }
    frame.render_widget(
        Paragraph::new(details)
            .style(Style::default().fg(TEXT).bg(BACKGROUND))
            .wrap(Wrap { trim: false }),
        rows[1],
    );

    let mut actions = if view.completion.is_some() {
        vec![key("Enter", "Continue")]
    } else {
        vec![key("R", "Retry"), key("O", "Open backup location")]
    };
    if view.rebuild_blocker.is_none() && view.completion.is_none() {
        actions.push(key("B", "Rebuild Cache"));
    }
    actions.push(key("Esc", "Exit"));
    frame.render_widget(
        Paragraph::new(Line::from(actions)).style(Style::default().bg(SURFACE_BACKGROUND)),
        rows[2],
    );
    frame.render_widget(
        Paragraph::new(" recovery · original Account data is never silently replaced")
            .style(Style::default().fg(MUTED_TEXT).bg(CHROME_BACKGROUND)),
        rows[3],
    );
}

fn key(label: &'static str, action: &'static str) -> Span<'static> {
    Span::styled(
        format!(" {label} {action}  "),
        Style::default()
            .fg(TEXT)
            .bg(SURFACE_BACKGROUND)
            .add_modifier(Modifier::BOLD),
    )
}

#[cfg(test)]
mod tests;
