use super::*;

mod media;
mod rich_text;

use media::{media_line_count, render_media};
use rich_text::{message_metadata, render_rich_text_lines};

pub(super) fn render_transcript(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &View,
    focused: bool,
    mode: ViewMode,
    semantics: &mut Vec<SemanticNode>,
) {
    let range = transcript_window(
        view,
        view.active_message.or(view.transcript_anchor),
        area.height,
        mode,
    );
    render_semantics(area, view, focused, range.clone(), mode, semantics);
    let items = view.messages[range.clone()]
        .iter()
        .enumerate()
        .map(|(offset, message)| {
            let index = range.start + offset;
            ListItem::new(message_lines(view, index, message, focused, mode))
        });
    frame.render_widget(List::new(items).style(surface_style(focused)), area);
}

fn message_lines(
    view: &View,
    index: usize,
    message: &MessageView,
    focused: bool,
    mode: ViewMode,
) -> Vec<Line<'static>> {
    let selected = view.active_message == Some(index);
    let direction = match message.direction {
        MessageDirection::Incoming => "←",
        MessageDirection::Outgoing => "→",
    };
    let delivery = match message.delivery {
        DeliveryState::Pending => "…",
        DeliveryState::Sent => "✓",
        DeliveryState::Read => "✓✓",
        DeliveryState::Failed => "!",
    };
    let reply = message
        .reply_to
        .map_or_else(String::new, |id| format!(" ↩{}", id.0));
    let forwarded = message
        .details
        .forwarded_from
        .as_ref()
        .map_or_else(String::new, |source| format!(" · forwarded from {source}"));
    let header = Line::from(vec![
        selection_rule(selected),
        Span::styled(
            format!("{direction} {}", message.sender),
            Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
        ),
        Span::styled(reply, Style::default().fg(MUTED_TEXT)),
        Span::styled(forwarded, Style::default().fg(MUTED_TEXT)),
        Span::raw("  "),
        Span::styled(
            format!("{} {delivery}", message.timestamp),
            Style::default().fg(MUTED_TEXT),
        ),
    ]);
    let mut body_lines = render_rich_text_lines(message);
    body_lines
        .last_mut()
        .expect("Message text always has at least one line")
        .extend(message_metadata(message));
    let mut lines = vec![header];
    lines.extend(body_lines.into_iter().map(|body| {
        Line::from(
            std::iter::once(selection_rule(selected))
                .chain(body)
                .collect::<Vec<_>>(),
        )
    }));
    if let Some(media) = &message.details.media {
        let preview = media_preview(view, message.id);
        lines.extend(render_media(
            media,
            preview,
            selected,
            focused,
            album_position(view, index, message.details.album_id),
        ));
    }
    if mode == ViewMode::Default {
        lines.push(Line::from(""));
    }
    lines
}

fn render_semantics(
    area: Rect,
    view: &View,
    focused: bool,
    range: std::ops::Range<usize>,
    mode: ViewMode,
    semantics: &mut Vec<SemanticNode>,
) {
    let mut y = area.y;
    for (offset, message) in view.messages[range.clone()].iter().enumerate() {
        let index = range.start + offset;
        let height = message_height(message, mode, media_preview(view, message.id))
            .min(area.bottom().saturating_sub(y));
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
        y = y.saturating_add(message_height(
            message,
            mode,
            media_preview(view, message.id),
        ));
    }
}

fn message_height(
    message: &MessageView,
    mode: ViewMode,
    preview: Option<&intuigram_app::InlineImage>,
) -> u16 {
    let body_height = u16::try_from(message.body.split('\n').count()).unwrap_or(u16::MAX);
    let media_height = message
        .details
        .media
        .as_ref()
        .map_or(0, |media| media_line_count(media, preview));
    mode.item_height(
        1_u16
            .saturating_add(body_height)
            .saturating_add(media_height),
    )
}

fn transcript_window(
    view: &View,
    active: Option<usize>,
    available: u16,
    mode: ViewMode,
) -> std::ops::Range<usize> {
    let messages = &view.messages;
    if messages.is_empty() {
        return 0..0;
    }
    let active = active.unwrap_or(messages.len() - 1).min(messages.len() - 1);
    let mut start = active;
    let mut before_height = 0_u16;
    let before_budget = available / 3;
    while start > 0 {
        let message = &messages[start - 1];
        let height = message_height(message, mode, media_preview(view, message.id));
        if before_height.saturating_add(height) > before_budget {
            break;
        }
        start -= 1;
        before_height = before_height.saturating_add(height);
    }
    let mut end = active + 1;
    let active_message = &messages[active];
    let mut used = before_height.saturating_add(message_height(
        active_message,
        mode,
        media_preview(view, active_message.id),
    ));
    while end < messages.len() {
        let message = &messages[end];
        let height = message_height(message, mode, media_preview(view, message.id));
        if used.saturating_add(height) > available {
            break;
        }
        used = used.saturating_add(height);
        end += 1;
    }
    while start > 0 {
        let message = &messages[start - 1];
        let height = message_height(message, mode, media_preview(view, message.id));
        if used.saturating_add(height) > available {
            break;
        }
        used = used.saturating_add(height);
        start -= 1;
    }
    start..end
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
