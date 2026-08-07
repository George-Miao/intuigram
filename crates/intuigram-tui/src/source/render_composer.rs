use super::composer_wrap::{WrappedText, wrap_text};
use super::render_chrome::interaction_rule;
use super::*;

const MAX_COMPOSER_HEIGHT: u16 = 9;
const RESERVED_TRANSCRIPT_HEIGHT: u16 = 5;

pub(super) fn composer_height(area: Rect, view: &View) -> u16 {
    if view.active_chat.is_none() {
        return 0;
    }
    let label = composer_label(view);
    let width = content_width(area.width, &label);
    let wrapped = wrap_text(&view.composer.text, view.composer.cursor, width);
    let desired = u16::try_from(wrapped.rows.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2)
        .max(3);
    let cap = area
        .height
        .saturating_sub(2 + RESERVED_TRANSCRIPT_HEIGHT)
        .clamp(3, MAX_COMPOSER_HEIGHT);
    desired.min(cap)
}

pub(super) fn render_composer(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &View,
    semantics: &mut Vec<SemanticNode>,
) {
    let focused = view.focus == Focus::Composer;
    semantics.push(SemanticNode {
        role: SemanticRole::Composer,
        name: "Composer".to_owned(),
        description: Some(view.composer.text.clone()),
        domain_id: None,
        action: None,
        delivery: None,
        active: true,
        selected: false,
        focused,
        bounds: area,
    });
    let label = composer_label(view);
    let label_width = Line::from(label.as_str()).width();
    let prefix_width = label_width.saturating_add(5);
    let width = content_width(area.width, &label);
    let wrapped = wrap_text(&view.composer.text, view.composer.cursor, width);
    let visible_height = usize::from(area.height.saturating_sub(2).max(1));
    let scroll = wrapped
        .cursor_row
        .saturating_add(1)
        .saturating_sub(visible_height);
    let lines = composer_lines(
        view,
        focused,
        &label,
        prefix_width,
        &wrapped,
        scroll,
        visible_height,
    );
    frame.render_widget(Paragraph::new(lines).style(surface_style(focused)), area);
    if focused && !overlay_open(view) {
        let x = area
            .x
            .saturating_add(u16::try_from(prefix_width).unwrap_or(u16::MAX))
            .saturating_add(u16::try_from(wrapped.cursor_column).unwrap_or(u16::MAX))
            .min(area.right().saturating_sub(1));
        let y = area
            .y
            .saturating_add(1)
            .saturating_add(u16::try_from(wrapped.cursor_row - scroll).unwrap_or(u16::MAX))
            .min(area.bottom().saturating_sub(2));
        frame.set_cursor_position((x, y));
    }
}

fn composer_lines(
    view: &View,
    focused: bool,
    label: &str,
    prefix_width: usize,
    wrapped: &WrappedText,
    scroll: usize,
    visible_height: usize,
) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from("")];
    for (visible_index, row) in wrapped
        .rows
        .iter()
        .skip(scroll)
        .take(visible_height)
        .enumerate()
    {
        let prefix = if visible_index == 0 {
            vec![
                Span::raw(" "),
                interaction_rule(focused),
                Span::styled(label.to_owned(), Style::default().fg(MUTED_TEXT)),
                Span::raw("  "),
            ]
        } else {
            vec![Span::raw(" ".repeat(prefix_width))]
        };
        let content = if view.composer.text.is_empty() {
            Span::styled("Type or paste a message…", Style::default().fg(MUTED_TEXT))
        } else {
            Span::raw(row.clone())
        };
        lines.push(Line::from(
            prefix
                .into_iter()
                .chain(std::iter::once(content))
                .collect::<Vec<_>>(),
        ));
    }
    lines.push(Line::from(""));
    lines
}

fn composer_label(view: &View) -> String {
    let label = if view.poll_composer {
        "Poll · question first, then one option per line".to_owned()
    } else {
        view.composer.editing.map_or_else(
            || {
                view.composer
                    .reply_to
                    .map_or_else(|| "Draft".to_owned(), |id| format!("Reply to {}", id.0))
            },
            |id| format!("Edit Message {}", id.0),
        )
    };
    if view.composer.attachments.is_empty() {
        label
    } else {
        format!(
            "{label} · {} attachment(s)",
            view.composer.attachments.len()
        )
    }
}

fn content_width(area_width: u16, label: &str) -> usize {
    usize::from(area_width)
        .saturating_sub(Line::from(label).width().saturating_add(5))
        .max(1)
}

fn overlay_open(view: &View) -> bool {
    view.help_open
        || view.attachment_path.is_some()
        || view.save_as.is_some()
        || view.link_confirmation.is_some()
        || view.reaction_picker.is_some()
        || view.poll_vote.is_some()
        || view.forward_picker.is_some()
        || view.delete_confirmation.is_some()
        || view.folder_picker.is_some()
}
