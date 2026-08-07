use super::*;

pub(super) fn message_metadata(message: &MessageView) -> Vec<Span<'static>> {
    let mut parts = Vec::new();
    if message.details.edited {
        parts.push("edited".to_owned());
    }
    if message.details.pinned {
        parts.push("pinned".to_owned());
    }
    if let Some(views) = message.details.views {
        parts.push(format!("{views} views"));
    }
    if let Some(forwards) = message.details.forwards {
        parts.push(format!("{forwards} forwards"));
    }
    if let Some(replies) = message.details.replies {
        parts.push(format!("{replies} replies"));
    }
    let mut spans = Vec::new();
    if !parts.is_empty() {
        spans.push(Span::styled(
            format!("  · {}", parts.join(" · ")),
            Style::default().fg(MUTED_TEXT),
        ));
    }
    for reaction in &message.details.reactions {
        let style = if reaction.chosen {
            Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(MUTED_TEXT)
        };
        spans.push(Span::styled(
            format!("  {} {}", reaction.label, reaction.count),
            style,
        ));
    }
    spans
}

pub(super) fn render_rich_text(message: &MessageView) -> Vec<Span<'static>> {
    if message.details.entities.is_empty() {
        return vec![message_span(
            message,
            message.body.clone(),
            Style::default(),
        )];
    }
    let mut result = Vec::new();
    let mut utf16_offset = 0;
    for character in message.body.chars() {
        let character_length = character.len_utf16();
        let mut style = Style::default();
        for entity in &message.details.entities {
            let entity_end = entity.offset.saturating_add(entity.length);
            if utf16_offset < entity_end && utf16_offset + character_length > entity.offset {
                style = entity_style(style, &entity.kind);
            }
        }
        result.push(message_span(message, character.to_string(), style));
        utf16_offset += character_length;
    }
    result
}

pub(super) fn render_rich_text_lines(message: &MessageView) -> Vec<Vec<Span<'static>>> {
    let mut lines = vec![Vec::new()];
    for span in render_rich_text(message) {
        let style = span.style;
        let parts = span.content.split('\n').collect::<Vec<_>>();
        let last = parts.len().saturating_sub(1);
        for (index, part) in parts.into_iter().enumerate() {
            if !part.is_empty() {
                lines
                    .last_mut()
                    .expect("rich text always has a current line")
                    .push(Span::styled(part.to_owned(), style));
            }
            if index < last {
                lines.push(Vec::new());
            }
        }
    }
    lines
}

fn message_span(message: &MessageView, text: String, style: Style) -> Span<'static> {
    if message.details.service.is_some() {
        Span::styled(text, style.fg(MUTED_TEXT).add_modifier(Modifier::ITALIC))
    } else {
        Span::styled(text, style)
    }
}

fn entity_style(style: Style, kind: &TextEntityKind) -> Style {
    match kind {
        TextEntityKind::Bold => style.add_modifier(Modifier::BOLD),
        TextEntityKind::Italic => style.add_modifier(Modifier::ITALIC),
        TextEntityKind::Underline => style.add_modifier(Modifier::UNDERLINED),
        TextEntityKind::Strike => style.add_modifier(Modifier::CROSSED_OUT),
        TextEntityKind::Code | TextEntityKind::Pre { .. } => {
            style.fg(SECONDARY).bg(SURFACE_BACKGROUND)
        }
        TextEntityKind::Spoiler => style.fg(MUTED_TEXT),
        TextEntityKind::Url | TextEntityKind::TextUrl { .. } => {
            style.fg(PRIMARY).add_modifier(Modifier::UNDERLINED)
        }
        TextEntityKind::Semantic | TextEntityKind::CustomEmoji { .. } => style.fg(PRIMARY),
    }
}
