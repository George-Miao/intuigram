use intuigram_lib::{ScheduledEditorOperation, ScheduledManagerView};

use super::*;

pub(in crate::source) const SCHEDULED_BINDINGS: &[Binding] = &[
    binding(
        KeyChord::plain(Key::Char('n')),
        "New",
        Action::NewScheduled,
        true,
    ),
    binding(
        KeyChord::plain(Key::Char('e')),
        "Edit",
        Action::EditScheduled,
        true,
    ),
    binding(
        KeyChord::plain(Key::Char('r')),
        "Reschedule",
        Action::RescheduleScheduled,
        true,
    ),
    binding(
        KeyChord::plain(Key::Char('d')),
        "Delete",
        Action::DeleteScheduled,
        true,
    ),
    binding(
        KeyChord::plain(Key::Char('s')),
        "Send Now",
        Action::SendScheduledNow,
        true,
    ),
    binding(
        KeyChord::plain(Key::Enter),
        "Save",
        Action::SaveScheduled,
        true,
    ),
    binding(
        KeyChord::plain(Key::Enter),
        "Confirm",
        Action::ConfirmScheduled,
        true,
    ),
];

pub(in crate::source) fn render_scheduled(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let Some(manager) = &view.scheduled else {
        return;
    };
    let popup = centered_rect(74, 68, area);
    if let Some(confirmation) = manager.confirmation {
        let action = if confirmation.send_now {
            "Send now"
        } else {
            "Delete"
        };
        overlays::render_overlay(
            frame,
            popup,
            vec![
                heading(format!(
                    "{action} Scheduled Message {}?",
                    confirmation.message.0
                )),
                Line::from(""),
                Line::from(Span::styled(
                    if confirmation.send_now {
                        "Telegram will deliver it immediately."
                    } else {
                        "The Message will be removed without being sent."
                    },
                    Style::default().fg(MUTED_TEXT),
                )),
            ],
        );
        return;
    }
    if let Some(editor) = &manager.editor {
        render_editor(frame, popup, manager, editor);
        return;
    }
    let title = if manager.pending {
        Line::from(effort_spans(
            "Updating Scheduled Messages",
            view.animation_frame,
        ))
    } else {
        heading("Scheduled Messages")
    };
    let mut lines = vec![
        title,
        Line::from(Span::styled(
            "Server-owned history for this Chat",
            Style::default().fg(MUTED_TEXT),
        )),
        Line::from(""),
    ];
    lines.extend(manager.messages.iter().enumerate().map(|(index, message)| {
        Line::from(vec![
            selection_rule(manager.selected == index),
            Span::styled(
                format!("{:<22}", message.delivery.editable()),
                Style::default().fg(MUTED_TEXT),
            ),
            Span::raw(message.summary.clone()),
        ])
    }));
    if manager.messages.is_empty() && !manager.pending {
        lines.push(Line::from(Span::styled(
            "No Scheduled Messages",
            Style::default().fg(MUTED_TEXT),
        )));
    }
    overlays::render_overlay(frame, popup, lines);
}

fn render_editor(
    frame: &mut Frame<'_>,
    popup: Rect,
    manager: &ScheduledManagerView,
    editor: &intuigram_lib::ScheduledEditorView,
) {
    let mut lines = vec![
        heading(match editor.operation {
            ScheduledEditorOperation::Create => "New Scheduled Message",
            ScheduledEditorOperation::Edit(_) => "Edit Scheduled Message",
            ScheduledEditorOperation::Reschedule(_) => "Reschedule Message",
        }),
        Line::from(""),
    ];
    match editor.operation {
        ScheduledEditorOperation::Create => {
            lines.push(field(editor.selected == 0, "Message", &editor.text));
            lines.push(field(editor.selected == 1, "Delivery", &editor.delivery));
        }
        ScheduledEditorOperation::Edit(_) => {
            lines.push(field(true, "Message", &editor.text));
        }
        ScheduledEditorOperation::Reschedule(_) => {
            lines.push(field(true, "Delivery", &editor.delivery));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Delivery accepts online or RFC 3339 with an explicit UTC offset.",
        Style::default().fg(MUTED_TEXT),
    )));
    overlays::render_overlay(frame, popup, lines);
    render_editor_cursor(frame, popup, manager);
}

fn heading(text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(
        text.into(),
        Style::default().add_modifier(Modifier::BOLD),
    ))
}

fn field(selected: bool, label: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        interaction_rule(selected),
        Span::styled(format!("{label:<12}"), Style::default().fg(MUTED_TEXT)),
        Span::raw(value.to_owned()),
    ])
}

fn render_editor_cursor(frame: &mut Frame<'_>, popup: Rect, manager: &ScheduledManagerView) {
    let Some(editor) = &manager.editor else {
        return;
    };
    let (row, value) = match (editor.operation, editor.selected) {
        (ScheduledEditorOperation::Create, 0) | (ScheduledEditorOperation::Edit(_), 0) => {
            (2, &editor.text)
        }
        (ScheduledEditorOperation::Create, 1) => (3, &editor.delivery),
        (ScheduledEditorOperation::Reschedule(_), 0) => (2, &editor.delivery),
        _ => return,
    };
    let content = overlays::popup_content_area(popup);
    if content.width == 0 || content.height == 0 {
        return;
    }
    let x = content
        .x
        .saturating_add(14)
        .saturating_add(u16::try_from(value.chars().count()).unwrap_or(u16::MAX))
        .min(content.right().saturating_sub(1));
    let y = content
        .y
        .saturating_add(row)
        .min(content.bottom().saturating_sub(1));
    frame.set_cursor_position((x, y));
}
