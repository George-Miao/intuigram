use super::*;

mod media;
mod rich_text;
mod window;

use media::{MediaRenderContext, render_media};
use rich_text::{message_metadata, render_rich_text_lines};
use window::{transcript_window, unread_boundary_index};

pub(super) fn render_transcript(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &View,
    focused: bool,
    mode: ViewMode,
    semantics: &mut Vec<SemanticNode>,
) {
    frame.render_widget(Paragraph::new("").style(surface_style(focused)), area);
    let area = mode.padded(area);
    if view.chat_loading == ChatLoadingState::Fresh && view.messages.is_empty() {
        render_fresh_loading(frame, area, view, focused);
        return;
    }
    let unread = unread_boundary_index(view);
    let rendered_messages = view
        .messages
        .iter()
        .enumerate()
        .map(|(index, message)| {
            message_lines(
                view,
                index,
                message,
                focused,
                mode,
                unread == Some(index),
                area.width,
            )
        })
        .collect::<Vec<_>>();
    let heights = rendered_messages
        .iter()
        .map(|lines| u16::try_from(lines.len()).unwrap_or(u16::MAX))
        .collect::<Vec<_>>();
    let range = transcript_window(
        &heights,
        view.active_message.or(view.transcript_anchor),
        area.height,
    );
    render_semantics(area, view, focused, range.clone(), &heights, semantics);
    let items = rendered_messages
        .into_iter()
        .skip(range.start)
        .take(range.len())
        .map(ListItem::new);
    frame.render_widget(List::new(items).style(surface_style(focused)), area);
}

fn render_fresh_loading(frame: &mut Frame<'_>, area: Rect, view: &View, focused: bool) {
    let line = Line::from(effort_spans("loading", view.animation_frame));
    let centered = Rect::new(
        area.x,
        area.y.saturating_add(area.height.saturating_sub(1) / 2),
        area.width,
        1.min(area.height),
    );
    frame.render_widget(
        Paragraph::new(line)
            .alignment(Alignment::Center)
            .style(surface_style(focused)),
        centered,
    );
}

fn message_lines(
    view: &View,
    index: usize,
    message: &MessageView,
    focused: bool,
    mode: ViewMode,
    unread: bool,
    width: u16,
) -> Vec<Line<'static>> {
    let selected = view.active_message == Some(index);
    let direction = match message.direction {
        MessageDirection::Incoming => "←",
        MessageDirection::Outgoing => "→",
    };
    let reply = message
        .reply_to
        .map_or_else(String::new, |id| format!(" ↩{}", id.0));
    let header = vec![
        selection_rule(selected),
        Span::styled(
            format!("{direction} {}", message.sender),
            Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
        ),
        Span::styled(reply, Style::default().fg(MUTED_TEXT)),
    ];
    let header = Line::from(header);
    let body_lines = render_rich_text_lines(message);
    let mut lines = Vec::new();
    if unread {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "Unread messages",
                Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD),
            ),
        ]));
    }
    lines.push(header);
    if let Some(source) = &message.details.forwarded_from {
        let mut provenance = content_prefix(selected, true);
        provenance.push(Span::styled(
            format!("Forwarded from {source}"),
            Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(provenance));
    }
    let forwarded = message.details.forwarded_from.is_some();
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
                    selected,
                    forwarded,
                    focused,
                    album: album_position(view, index, message.details.album_id),
                    animation_frame: view.animation_frame,
                },
            )
        });
    let body_lines = body_lines.into_iter().map(|body| {
        Line::from(
            content_prefix(selected, forwarded)
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
    append_message_metadata(
        &mut lines,
        message,
        view.animation_frame,
        width,
        selected,
        forwarded,
    );
    if mode == ViewMode::Default {
        lines.push(Line::from(""));
    }
    lines
}

fn append_message_metadata(
    lines: &mut Vec<Line<'static>>,
    message: &MessageView,
    animation_frame: u8,
    width: u16,
    selected: bool,
    forwarded: bool,
) {
    let metadata = message_metadata(message, animation_frame);
    let metadata_width = Line::from(metadata.clone()).width();
    let line = lines
        .last_mut()
        .expect("Every Message has a sender header and content line");
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

    let mut spans = content_prefix(selected, forwarded);
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

fn content_prefix(selected: bool, forwarded: bool) -> Vec<Span<'static>> {
    let mut spans = vec![selection_rule(selected)];
    if forwarded {
        spans.push(Span::styled("│ ", Style::default().fg(PRIMARY)));
    }
    spans
}

fn render_semantics(
    area: Rect,
    view: &View,
    focused: bool,
    range: std::ops::Range<usize>,
    heights: &[u16],
    semantics: &mut Vec<SemanticNode>,
) {
    let mut y = area.y;
    for (offset, message) in view.messages[range.clone()].iter().enumerate() {
        let index = range.start + offset;
        let height = heights[index].min(area.bottom().saturating_sub(y));
        semantics.push(SemanticNode {
            role: SemanticRole::Message,
            name: message.body.clone(),
            description: Some(message.sender.clone()),
            domain_id: Some(message.id.0),
            action: None,
            delivery: Some(message.delivery),
            active: view.active_message == Some(index),
            focused,
            bounds: Rect::new(area.x, y, area.width, height),
        });
        if let Some(media) = &message.details.media {
            semantics.push(SemanticNode {
                role: SemanticRole::MediaCard,
                name: media.title.clone(),
                description: Some(media.description.clone()),
                domain_id: media.remote_id.as_ref().and_then(|id| id.parse().ok()),
                action: None,
                delivery: None,
                active: view.active_message == Some(index),
                focused,
                bounds: Rect::new(
                    area.x,
                    y.saturating_add(2),
                    area.width,
                    height.saturating_sub(2),
                ),
            });
        }
        y = y.saturating_add(heights[index]);
    }
}

fn media_preview(
    view: &View,
    message: intuigram_app::MessageId,
) -> Option<&intuigram_app::InlineImage> {
    let chat = active_chat(view)?;
    view.downloads
        .iter()
        .find(|download| download.chat == chat && download.message == message)
        .and_then(|download| download.preview.as_ref())
        .or_else(|| {
            view.media_previews
                .iter()
                .find(|preview| preview.chat == chat && preview.message == message)
                .map(|preview| &preview.image)
        })
}

fn media_preview_is_loading(view: &View, message: intuigram_app::MessageId) -> bool {
    let Some(chat) = active_chat(view) else {
        return false;
    };
    view.media_preview_loads
        .iter()
        .any(|loading| loading.chat == chat && loading.message == message)
}

fn active_chat(view: &View) -> Option<intuigram_app::ChatId> {
    view.active_chat
        .and_then(|index| view.chats.get(index))
        .map(|chat| chat.id)
}

fn album_position(view: &View, index: usize, album: Option<i64>) -> media::AlbumPosition {
    let Some(album) = album else {
        return media::AlbumPosition::None;
    };
    let before = index
        .checked_sub(1)
        .and_then(|index| view.messages.get(index));
    let after = view.messages.get(index + 1);
    media::AlbumPosition::from_neighbors(
        before.is_some_and(|message| message.details.album_id == Some(album)),
        after.is_some_and(|message| message.details.album_id == Some(album)),
    )
}
