use super::*;

pub(super) fn message_metadata(message: &MessageView, animation_frame: u8) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    if let Some(views) = message.details.views {
        push_metadata(
            &mut spans,
            format!("{views} views"),
            Style::default().fg(MUTED_TEXT),
        );
    }
    if let Some(forwards) = message.details.forwards.filter(|count| *count > 0) {
        push_metadata(
            &mut spans,
            format!("{forwards} forwards"),
            Style::default().fg(MUTED_TEXT),
        );
    }
    if let Some(replies) = message.details.replies.filter(|count| *count > 0) {
        push_metadata(
            &mut spans,
            format!("{replies} replies"),
            Style::default().fg(MUTED_TEXT),
        );
    }
    for reaction in message
        .details
        .reactions
        .iter()
        .filter(|reaction| reaction.count > 0)
    {
        let style = if reaction.chosen {
            Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(MUTED_TEXT)
        };
        push_metadata(
            &mut spans,
            format!("{} {}", reaction.label, reaction.count),
            style,
        );
    }
    if message.details.edited {
        push_metadata(&mut spans, "edited", Style::default().fg(MUTED_TEXT));
    }
    if message.details.pinned {
        push_metadata(&mut spans, "pinned", Style::default().fg(MUTED_TEXT));
    }
    push_metadata(
        &mut spans,
        message.timestamp.clone(),
        Style::default().fg(MUTED_TEXT),
    );
    match message.delivery {
        DeliveryState::Pending => {
            if !spans.is_empty() {
                spans.push(Span::styled(" · ", Style::default().fg(MUTED_TEXT)));
            }
            spans.extend(effort_spans("sending…", animation_frame));
        }
        DeliveryState::Sent => push_metadata(&mut spans, "✓", Style::default().fg(MUTED_TEXT)),
        DeliveryState::Read => push_metadata(&mut spans, "✓✓", Style::default().fg(MUTED_TEXT)),
        DeliveryState::Failed => {
            push_metadata(&mut spans, "failed !", Style::default().fg(MUTED_TEXT))
        }
    }
    spans
}

fn push_metadata(spans: &mut Vec<Span<'static>>, text: impl Into<String>, style: Style) {
    if !spans.is_empty() {
        spans.push(Span::styled(" · ", Style::default().fg(MUTED_TEXT)));
    }
    spans.push(Span::styled(text.into(), style));
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
