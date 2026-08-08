use super::media::{MediaRenderContext, render_media};
use super::rich_text::{message_metadata, render_rich_text_lines};
use super::*;

pub(super) struct MessageLayout {
    pub(super) focused: bool,
    pub(super) mode: ViewMode,
    pub(super) unread: bool,
    pub(super) width: u16,
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
    };
    let mut lines = message_heading(message, state, &layout);
    if let Some(source) = &message.details.forwarded_from {
        let mut provenance = content_prefix(state.active, state.selected, true);
        provenance.push(Span::styled(
            format!("Forwarded from {source}"),
            Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(provenance));
    }
    if let Some(reply) = message.reply_to {
        let mut spans = content_prefix(state.active, state.selected, state.forwarded);
        spans.push(Span::styled("│ ", Style::default().fg(SECONDARY)));
        spans.push(Span::styled(
            reply_preview(view, reply),
            Style::default().fg(MUTED_TEXT),
        ));
        lines.push(Line::from(spans));
    }
    append_content(view, index, message, &layout, state, &mut lines, graphics);
    append_message_metadata(
        &mut lines,
        message,
        view.animation_frame,
        layout.width,
        state,
    );
    if layout.mode == ViewMode::Default && !layout.grouped_with_next {
        lines.push(Line::from(""));
    }
    lines
}

fn message_heading(
    message: &MessageView,
    state: MessageState,
    layout: &MessageLayout,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if layout.unread {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "Unread messages",
                Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD),
            ),
        ]));
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
        let direction = match message.direction {
            MessageDirection::Incoming => "←",
            MessageDirection::Outgoing => "→",
        };
        lines.push(Line::from(vec![
            selection_rule(state.active),
            message_selection_marker(state.selected),
            avatar_badge(&message.sender),
            Span::styled(
                format!("{direction} {}", message.sender),
                Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
            ),
        ]));
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
                    max_height: layout.available_height.saturating_sub(3).max(1),
                },
                active_chat(view).map(|chat| image_id(chat, message.id)),
                graphics,
            )
        });
    let body_lines = render_rich_text_lines(message).into_iter().map(|body| {
        Line::from(
            content_prefix(state.active, state.selected, state.forwarded)
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
    let mut spans = content_prefix(state.active, state.selected, state.forwarded);
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

pub(super) fn content_prefix(active: bool, selected: bool, forwarded: bool) -> Vec<Span<'static>> {
    let mut spans = vec![selection_rule(active), message_selection_marker(selected)];
    if forwarded {
        spans.push(Span::styled("│ ", Style::default().fg(PRIMARY)));
    }
    spans
}

fn message_selection_marker(selected: bool) -> Span<'static> {
    if selected {
        Span::styled("[✓] ", Style::default().fg(PRIMARY))
    } else {
        Span::raw("")
    }
}
