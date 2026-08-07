use super::*;

mod media;
mod rich_text;
mod window;

use media::{media_line_count, render_media};
use rich_text::{message_metadata, render_rich_text_lines};
use window::{message_height, transcript_window, unread_boundary_index};

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
    let unread = unread_boundary_index(view);
    let range = transcript_window(
        view,
        view.active_message.or(view.transcript_anchor),
        area.height,
        mode,
        unread,
    );
    render_semantics(area, view, focused, range.clone(), mode, unread, semantics);
    let items = view.messages[range.clone()]
        .iter()
        .enumerate()
        .map(|(offset, message)| {
            let index = range.start + offset;
            ListItem::new(message_lines(
                view,
                index,
                message,
                focused,
                mode,
                unread == Some(index),
            ))
        });
    frame.render_widget(List::new(items).style(surface_style(focused)), area);
}

fn message_lines(
    view: &View,
    index: usize,
    message: &MessageView,
    focused: bool,
    mode: ViewMode,
    unread: bool,
) -> Vec<Line<'static>> {
    let selected = view.active_message == Some(index);
    let direction = match message.direction {
        MessageDirection::Incoming => "←",
        MessageDirection::Outgoing => "→",
    };
    let reply = message
        .reply_to
        .map_or_else(String::new, |id| format!(" ↩{}", id.0));
    let forwarded = message
        .details
        .forwarded_from
        .as_ref()
        .map_or_else(String::new, |source| format!(" · forwarded from {source}"));
    let mut header = vec![
        selection_rule(selected),
        Span::styled(
            format!("{direction} {}", message.sender),
            Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
        ),
        Span::styled(reply, Style::default().fg(MUTED_TEXT)),
        Span::styled(forwarded, Style::default().fg(MUTED_TEXT)),
        Span::raw("  "),
        Span::styled(message.timestamp.clone(), Style::default().fg(MUTED_TEXT)),
        Span::raw(" "),
    ];
    match message.delivery {
        DeliveryState::Pending => {
            header.extend(effort_spans("sending…", view.animation_frame));
        }
        DeliveryState::Sent => header.push(Span::styled("✓", Style::default().fg(MUTED_TEXT))),
        DeliveryState::Read => header.push(Span::styled("✓✓", Style::default().fg(MUTED_TEXT))),
        DeliveryState::Failed => header.push(Span::styled("!", Style::default().fg(MUTED_TEXT))),
    }
    let header = Line::from(header);
    let mut body_lines = render_rich_text_lines(message);
    body_lines
        .last_mut()
        .expect("Message text always has at least one line")
        .extend(message_metadata(message));
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
    unread: Option<usize>,
    semantics: &mut Vec<SemanticNode>,
) {
    let mut y = area.y;
    for (offset, message) in view.messages[range.clone()].iter().enumerate() {
        let index = range.start + offset;
        let height = message_height(
            message,
            mode,
            media_preview(view, message.id),
            unread == Some(index),
        )
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
            unread == Some(index),
        ));
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
