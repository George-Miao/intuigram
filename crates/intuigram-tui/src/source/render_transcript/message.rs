use super::media::{MediaRenderContext, render_media};
use super::rich_text::{message_metadata, render_rich_text_lines};
use super::*;

pub(super) struct MessageLayout {
    pub(super) focused: bool,
    pub(super) mode: ViewMode,
    pub(super) unread: bool,
    pub(super) content_width: u16,
    pub(super) transcript_width: u16,
    pub(super) available_height: u16,
    pub(super) grouped_with_previous: bool,
    pub(super) grouped_with_next: bool,
    pub(super) date_boundary: bool,
}

#[derive(Clone, Copy)]
struct MessageState {
    active: bool,
    selected: bool,
    forwarded: bool,
    content_indent: usize,
}

pub(super) fn message_lines(
    view: &View,
    index: usize,
    message: &MessageView,
    layout: MessageLayout,
    graphics: &mut GraphicsFrame,
) -> Vec<Line<'static>> {
    let state = MessageState {
        active: view.active_message == Some(index),
        selected: view.selected_messages.contains(&message.id),
        forwarded: message.details.forwarded_from.is_some(),
        content_indent: avatar_width(view, message.details.sender_peer, &message.sender),
    };
    let mut lines = message_heading(view, message, state, &layout, graphics);
    if let Some(source) = &message.details.forwarded_from {
        lines.push(message_spacing(state.active));
        let mut provenance =
            content_prefix(state.active, state.selected, true, state.content_indent);
        provenance.push(Span::styled(
            format!("Forwarded from {source}"),
            Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(provenance));
    }
    if let Some(reply) = message.reply_to {
        lines.push(message_spacing(state.active));
        let mut spans = content_prefix(
            state.active,
            state.selected,
            state.forwarded,
            state.content_indent,
        );
        spans.push(Span::styled("│ ", Style::default().fg(SECONDARY)));
        spans.push(Span::styled(
            reply_preview(view, reply),
            Style::default().fg(MUTED_TEXT),
        ));
        lines.push(Line::from(spans));
        lines.push(message_spacing(state.active));
    }
    append_content(view, index, message, &layout, state, &mut lines, graphics);
    append_message_metadata(
        &mut lines,
        message,
        view.animation_frame,
        layout.transcript_width,
        state,
    );
    if layout.mode == ViewMode::Default && !layout.grouped_with_next {
        lines.push(message_spacing(state.active));
    }
    lines
}

fn message_heading(
    view: &View,
    message: &MessageView,
    state: MessageState,
    layout: &MessageLayout,
    graphics: &mut GraphicsFrame,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if layout.unread {
        lines.push(
            Line::from(Span::styled(
                "Unread messages",
                Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Center),
        );
    }
    if layout.date_boundary {
        lines.push(
            Line::from(Span::styled(
                message.details.date_label.clone(),
                Style::default().fg(MUTED_TEXT).add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Center),
        );
    }
    if !layout.grouped_with_previous {
        let avatar_id = active_chat(view)
            .zip(message.details.sender_peer)
            .map(|(chat, peer)| avatar_image_id(peer, chat.0 ^ message.id.0 ^ 0x4d53_4741));
        let mut heading = vec![
            selection_rule(state.active),
            message_selection_marker(state.selected),
        ];
        heading.extend(avatar_spans(
            view,
            message.details.sender_peer,
            &message.sender,
            avatar_id,
            graphics,
            layout.focused,
        ));
        heading.push(Span::styled(
            message.sender.clone(),
            Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(heading));
    }
    lines
}

fn append_content(
    view: &View,
    index: usize,
    message: &MessageView,
    layout: &MessageLayout,
    state: MessageState,
    lines: &mut Vec<Line<'static>>,
    graphics: &mut GraphicsFrame,
) {
    let preview = media_preview(view, message.id);
    let loading = media_preview_is_loading(view, message.id);
    let inline_media = message.details.media.is_some() && (preview.is_some() || loading);
    let show_body = !inline_media || !body_is_media_fallback(message);
    let prefix = content_prefix(
        state.active,
        state.selected,
        state.forwarded,
        state.content_indent,
    );
    let content_width =
        usize::from(layout.content_width).saturating_sub(Line::from(prefix.as_slice()).width());
    let media_lines = message
        .details
        .media
        .as_ref()
        .map_or_else(Vec::new, |media| {
            render_media(
                media,
                preview,
                loading,
                MediaRenderContext {
                    active: state.active,
                    selected: state.selected,
                    forwarded: state.forwarded,
                    focused: layout.focused,
                    album: album_position(view, index, message.details.album_id),
                    animation_frame: view.animation_frame,
                    max_width: u16::try_from(content_width).unwrap_or(u16::MAX).max(1),
                    max_height: layout.available_height.saturating_sub(5).max(1),
                    content_indent: state.content_indent,
                },
                active_chat(view).map(|chat| image_id(chat, message.id)),
                graphics,
            )
        });
    let body_lines = show_body
        .then(|| render_rich_text_lines(message, content_width))
        .into_iter()
        .flatten()
        .map(|body| {
            Line::from(
                content_prefix(
                    state.active,
                    state.selected,
                    state.forwarded,
                    state.content_indent,
                )
                .into_iter()
                .chain(body)
                .collect::<Vec<_>>(),
            )
        });
    if inline_media {
        lines.extend(media_lines);
        lines.extend(body_lines);
    } else {
        lines.extend(body_lines);
        lines.extend(media_lines);
    }
}

fn body_is_media_fallback(message: &MessageView) -> bool {
    message
        .details
        .media
        .as_ref()
        .is_some_and(|media| media.is_fallback_body(&message.body))
}

pub(super) fn messages_group(previous: &MessageView, current: &MessageView) -> bool {
    previous.sender == current.sender
        && previous.direction == current.direction
        && previous.details.date_label == current.details.date_label
        && previous.details.service.is_none()
        && current.details.service.is_none()
        && current.details.forwarded_from.is_none()
}

fn reply_preview(view: &View, reply: intuigram_app::MessageId) -> String {
    view.messages
        .iter()
        .find(|message| message.id == reply)
        .map_or_else(
            || format!("Message #{} is not loaded", reply.0),
            |message| {
                let body = message.body.replace('\n', " ");
                let preview = body.chars().take(48).collect::<String>();
                format!("{}: {preview}", message.sender)
            },
        )
}

fn append_message_metadata(
    lines: &mut Vec<Line<'static>>,
    message: &MessageView,
    animation_frame: u8,
    width: u16,
    state: MessageState,
) {
    let metadata = message_metadata(message, animation_frame);
    let metadata_width = Line::from(metadata.clone()).width();
    let line = lines.last_mut().expect("Every Message has a content line");
    let line_width = line.width();
    let width = usize::from(width);
    if line_width.saturating_add(metadata_width).saturating_add(2) <= width {
        line.push_span(Span::raw(
            " ".repeat(
                width
                    .saturating_sub(line_width)
                    .saturating_sub(metadata_width),
            ),
        ));
        line.extend(metadata);
        return;
    }
    let mut spans = content_prefix(
        state.active,
        state.selected,
        state.forwarded,
        state.content_indent,
    );
    let prefix_width = Line::from(spans.clone()).width();
    spans.push(Span::raw(
        " ".repeat(
            width
                .saturating_sub(prefix_width)
                .saturating_sub(metadata_width),
        ),
    ));
    spans.extend(metadata);
    lines.push(Line::from(spans));
}

pub(super) fn content_prefix(
    active: bool,
    selected: bool,
    forwarded: bool,
    indent: usize,
) -> Vec<Span<'static>> {
    let mut spans = vec![selection_rule(active), message_selection_marker(selected)];
    if forwarded {
        spans.push(Span::styled("│ ", Style::default().fg(PRIMARY)));
    }
    spans.push(Span::raw(" ".repeat(indent)));
    spans
}

pub(super) fn message_spacing(active: bool) -> Line<'static> {
    Line::from(selection_rule(active))
}

fn message_selection_marker(selected: bool) -> Span<'static> {
    if selected {
        Span::styled("[✓] ", Style::default().fg(PRIMARY))
    } else {
        Span::raw("")
    }
}
