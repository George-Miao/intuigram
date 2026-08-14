use super::media::{MediaRenderContext, render_media};
use super::rich_text::{message_metadata, render_rich_text_lines};
use super::*;
use crate::source::render::outbox::message_outbox;

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
    component: MessageComponent,
}

#[derive(Clone, Copy)]
struct ContentContext<'a> {
    layout: &'a MessageLayout,
    message: MessageState,
}

#[derive(Clone, Copy)]
pub(super) enum MessageComponent {
    Plain {
        avatar_padding: usize,
    },
    Forwarded {
        avatar_padding: usize,
    },
    Reply {
        avatar_padding: usize,
        forwarded: bool,
    },
}

impl MessageComponent {
    pub(super) fn prefix(self, active: bool, selected: bool) -> Vec<Span<'static>> {
        self.prefix_with_avatar(active, selected, None)
    }

    fn prefix_with_avatar(
        self,
        active: bool,
        selected: bool,
        avatar: Option<Vec<Span<'static>>>,
    ) -> Vec<Span<'static>> {
        let mut spans = vec![selection_rule(active), message_selection_marker(selected)];
        if let Some(avatar) = avatar {
            spans.extend(avatar);
        } else {
            let avatar_padding = match self {
                Self::Plain { avatar_padding }
                | Self::Forwarded { avatar_padding }
                | Self::Reply { avatar_padding, .. } => avatar_padding,
            };
            spans.push(Span::raw(" ".repeat(avatar_padding)));
        }
        match self {
            Self::Plain { .. }
            | Self::Reply {
                forwarded: false, ..
            } => {}
            Self::Forwarded { .. }
            | Self::Reply {
                forwarded: true, ..
            } => {
                spans.push(Span::styled("│ ", Style::default().fg(PRIMARY)));
            }
        }
        spans
    }

    const fn is_forwarded(self) -> bool {
        matches!(self, Self::Forwarded { .. })
    }
}

pub(super) fn message_lines(
    view: &View,
    index: usize,
    message: &MessageView,
    layout: MessageLayout,
    graphics: &mut GraphicsFrame,
) -> Vec<Line<'static>> {
    let avatar_padding = avatar_width(graphics, 2);
    let component = if message.details.forwarded_from.is_some() {
        MessageComponent::Forwarded { avatar_padding }
    } else {
        MessageComponent::Plain { avatar_padding }
    };
    let state = MessageState {
        active: view.active_message == Some(index),
        selected: view.selected_messages.contains(&message.id),
        component,
    };
    let avatar = (!layout.grouped_with_previous).then(|| {
        let id = active_chat(view)
            .zip(message.details.sender_peer)
            .map(|(chat, peer)| avatar_image_id(peer, chat.0 ^ message.id.0 ^ 0x4d53_4741));
        avatar_block(
            view,
            message.details.sender_peer,
            &message.sender,
            id,
            graphics,
            layout.focused,
        )
    });
    let (avatar_top, mut avatar_bottom) =
        avatar.map_or((None, None), |[top, bottom]| (Some(top), Some(bottom)));
    let mut lines = message_heading(message, state, &layout, avatar_top);
    if let Some(source) = &message.details.forwarded_from {
        if avatar_bottom.is_none() {
            lines.push(message_spacing(state.active));
        }
        let mut provenance = MessageComponent::Forwarded { avatar_padding }.prefix_with_avatar(
            state.active,
            state.selected,
            avatar_bottom.take(),
        );
        provenance.push(Span::styled(
            format!("Forwarded from {source}"),
            Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(provenance));
    }
    if let Some(reply) = message.reply_to {
        if avatar_bottom.is_none() {
            lines.push(message_spacing(state.active));
        }
        let reply_component = MessageComponent::Reply {
            avatar_padding,
            forwarded: state.component.is_forwarded(),
        };
        let mut spans =
            reply_component.prefix_with_avatar(state.active, state.selected, avatar_bottom.take());
        spans.push(Span::styled("│ ", Style::default().fg(SECONDARY)));
        spans.push(Span::styled(
            reply_preview(view, reply),
            Style::default().fg(MUTED_TEXT),
        ));
        lines.push(Line::from(spans));
    }
    if state.component.is_forwarded() {
        lines.push(Line::from(
            state.component.prefix(state.active, state.selected),
        ));
    }
    let content = ContentContext {
        layout: &layout,
        message: state,
    };
    let content_start = lines.len();
    append_content(view, index, message, content, &mut lines, graphics);
    if let Some(bottom) = avatar_bottom.take() {
        let line = lines
            .get_mut(content_start)
            .expect("Every Message has a content line below its sender heading");
        place_avatar_row(line, state, bottom);
    }
    append_message_metadata(
        &mut lines,
        message,
        message_outbox(view, message),
        view.animation_frame,
        layout.transcript_width,
        state,
    );
    if layout.mode == ViewMode::Default && !layout.grouped_with_next {
        lines.push(message_spacing(false));
    }
    lines
}

fn message_heading(
    message: &MessageView,
    state: MessageState,
    layout: &MessageLayout,
    avatar: Option<Vec<Span<'static>>>,
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
        let mut heading = vec![
            selection_rule(state.active),
            message_selection_marker(state.selected),
        ];
        heading.extend(avatar.unwrap_or_default());
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
    context: ContentContext<'_>,
    lines: &mut Vec<Line<'static>>,
    graphics: &mut GraphicsFrame,
) {
    let ContentContext {
        layout,
        message: state,
    } = context;
    let component = state.component;
    let prefix = component.prefix(state.active, state.selected);
    let content_width =
        usize::from(layout.content_width).saturating_sub(Line::from(prefix.as_slice()).width());
    let preview = media_preview(view, message.id);
    let loading = media_preview_is_loading(view, message.id);
    let inline_media = message.details.media.is_some() && (preview.is_some() || loading);
    let show_body = !body_is_media_fallback(message);
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
                    component,
                    focused: layout.focused,
                    album: album_position(view, index, message.details.album_id),
                    animation_frame: view.animation_frame,
                    max_width: u16::try_from(content_width).unwrap_or(u16::MAX).max(1),
                    max_height: layout.available_height.saturating_sub(5).max(1),
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
                component
                    .prefix(state.active, state.selected)
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

fn reply_preview(view: &View, reply: intuigram_lib::MessageId) -> String {
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
    outbox: Option<&intuigram_lib::OutboxItemView>,
    animation_frame: u8,
    width: u16,
    state: MessageState,
) {
    let component = state.component;
    let width = usize::from(width);
    let prefix_width = Line::from(component.prefix(state.active, state.selected)).width();
    let metadata = message_metadata(
        message,
        outbox,
        animation_frame,
        width.saturating_sub(prefix_width),
    );
    let metadata_width = Line::from(metadata.clone()).width();
    let line = lines.last_mut().expect("Every Message has a content line");
    let line_width = line.width();
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
    let mut spans = component.prefix(state.active, state.selected);
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

fn place_avatar_row(line: &mut Line<'static>, state: MessageState, avatar: Vec<Span<'static>>) {
    let prefix_len = state.component.prefix(state.active, state.selected).len();
    let tail = line.spans.split_off(prefix_len);
    line.spans = state
        .component
        .prefix_with_avatar(state.active, state.selected, Some(avatar));
    line.spans.extend(tail);
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
