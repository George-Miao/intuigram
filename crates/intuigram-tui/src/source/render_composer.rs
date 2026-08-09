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
    let width = content_width(area.width, label.as_deref());
    let wrapped = wrap_text(&view.composer.text, view.composer.cursor, width);
    let context_height = u16::from(editing_preview(view).is_some());
    let desired = u16::try_from(wrapped.rows.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2)
        .saturating_add(context_height)
        .max(3_u16.saturating_add(context_height));
    let cap = area
        .height
        .saturating_sub(2 + RESERVED_TRANSCRIPT_HEIGHT)
        .clamp(3_u16.saturating_add(context_height), MAX_COMPOSER_HEIGHT);
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
    let edit_preview = editing_preview(view);
    let context_height = usize::from(edit_preview.is_some());
    let prefix_width = composer_prefix_width(label.as_deref());
    let width = content_width(area.width, label.as_deref());
    let wrapped = wrap_text(&view.composer.text, view.composer.cursor, width);
    let visible_height = usize::from(
        area.height
            .saturating_sub(2)
            .saturating_sub(u16::try_from(context_height).unwrap_or(u16::MAX))
            .max(1),
    );
    let scroll = wrapped
        .cursor_row
        .saturating_add(1)
        .saturating_sub(visible_height);
    let lines = composer_lines(
        focused,
        label.as_deref(),
        edit_preview.as_deref(),
        view.composer.text.is_empty(),
        prefix_width,
        &wrapped,
        scroll..scroll.saturating_add(visible_height),
    );
    frame.render_widget(Paragraph::new("").style(surface_style(focused)), area);
    let content_area = Rect::new(area.x, area.y, area.width.saturating_sub(1), area.height);
    frame.render_widget(
        Paragraph::new(lines).style(surface_style(focused)),
        content_area,
    );
    if focused && !overlay_open(view) {
        let x = area
            .x
            .saturating_add(u16::try_from(prefix_width).unwrap_or(u16::MAX))
            .saturating_add(u16::try_from(wrapped.cursor_column).unwrap_or(u16::MAX))
            .min(area.right().saturating_sub(2));
        let y = area
            .y
            .saturating_add(1)
            .saturating_add(u16::try_from(context_height).unwrap_or(u16::MAX))
            .saturating_add(u16::try_from(wrapped.cursor_row - scroll).unwrap_or(u16::MAX))
            .min(area.bottom().saturating_sub(2));
        frame.set_cursor_position((x, y));
    }
}

pub(super) fn composer_cursor_at(view: &View, area: Rect, column: u16, row: u16) -> Option<usize> {
    let label = composer_label(view);
    let context_height = usize::from(editing_preview(view).is_some());
    let prefix_width = composer_prefix_width(label.as_deref());
    let width = content_width(area.width, label.as_deref());
    let wrapped = wrap_text(&view.composer.text, view.composer.cursor, width);
    let visible_height = usize::from(
        area.height
            .saturating_sub(2)
            .saturating_sub(u16::try_from(context_height).unwrap_or(u16::MAX))
            .max(1),
    );
    let scroll = wrapped
        .cursor_row
        .saturating_add(1)
        .saturating_sub(visible_height);
    let text_y = area
        .y
        .saturating_add(1)
        .saturating_add(u16::try_from(context_height).unwrap_or(u16::MAX));
    let visible_row = usize::from(row.checked_sub(text_y)?);
    if visible_row >= visible_height {
        return None;
    }
    let text_column = usize::from(
        column.saturating_sub(
            area.x
                .saturating_add(u16::try_from(prefix_width).unwrap_or(u16::MAX)),
        ),
    );
    wrapped.cursor_at(
        &view.composer.text,
        scroll.saturating_add(visible_row),
        text_column,
    )
}

fn composer_lines(
    focused: bool,
    label: Option<&str>,
    edit_preview: Option<&str>,
    composer_is_empty: bool,
    prefix_width: usize,
    wrapped: &WrappedText,
    visible_rows: std::ops::Range<usize>,
) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from("")];
    if let Some(preview) = edit_preview {
        lines.push(Line::from(vec![
            Span::raw(" "),
            interaction_rule(focused),
            Span::styled("Edit · ", Style::default().fg(MUTED_TEXT)),
            Span::raw(preview.to_owned()),
        ]));
    }
    for (visible_index, row) in wrapped
        .rows
        .iter()
        .skip(visible_rows.start)
        .take(visible_rows.len())
        .enumerate()
    {
        let prefix = if visible_index == 0 {
            let mut prefix = vec![Span::raw(" "), interaction_rule(focused)];
            if let Some(label) = label {
                prefix.push(Span::styled(
                    label.to_owned(),
                    Style::default().fg(MUTED_TEXT),
                ));
                prefix.push(Span::raw("  "));
            }
            prefix
        } else {
            vec![Span::raw(" ".repeat(prefix_width))]
        };
        let content = if composer_is_empty {
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

fn composer_label(view: &View) -> Option<String> {
    let label = if view.poll_composer {
        Some("Poll · question first, then one option per line".to_owned())
    } else if view.composer.editing.is_some() {
        None
    } else {
        view.composer
            .reply_to
            .map(|id| format!("Reply to {}", id.0))
    };
    if view.composer.attachments.is_empty() {
        label
    } else {
        Some(label.map_or_else(
            || format!("{} attachment(s)", view.composer.attachments.len()),
            |label| {
                format!(
                    "{label} · {} attachment(s)",
                    view.composer.attachments.len()
                )
            },
        ))
    }
}

fn editing_preview(view: &View) -> Option<String> {
    let editing = view.composer.editing?;
    let message = view.messages.iter().find(|message| message.id == editing)?;
    let preview = message
        .body
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    Some(if preview.is_empty() {
        "message".to_owned()
    } else {
        preview
    })
}

fn composer_prefix_width(label: Option<&str>) -> usize {
    label.map_or(3, |label| Line::from(label).width().saturating_add(5))
}

fn content_width(area_width: u16, label: Option<&str>) -> usize {
    usize::from(area_width)
        .saturating_sub(composer_prefix_width(label).saturating_add(1))
        .max(1)
}

fn overlay_open(view: &View) -> bool {
    view.help_open
        || view.action_menu.is_some()
        || view.scheduled.is_some()
        || view.rich_media.is_some()
        || view.attachment_path.is_some()
        || view.save_as.is_some()
        || view.link_confirmation.is_some()
        || view.reaction_picker.is_some()
        || view.poll_vote.is_some()
        || view.forward_picker.is_some()
        || view.delete_confirmation.is_some()
        || view.folder_picker.is_some()
}
