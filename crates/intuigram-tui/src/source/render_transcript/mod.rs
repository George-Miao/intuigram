use super::*;

mod media;
mod media_image;
mod message;
mod rich_text;
mod window;

use message::{MessageLayout, content_prefix, message_lines, messages_group};
use window::{transcript_window, unread_boundary_index};

pub(super) fn render_transcript(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &View,
    focused: bool,
    mode: ViewMode,
    semantics: &mut Vec<SemanticNode>,
    graphics: &mut GraphicsFrame,
) {
    frame.render_widget(Paragraph::new("").style(surface_style(focused)), area);
    let area = mode.padded(area);
    semantics.push(SemanticNode {
        role: SemanticRole::Transcript,
        name: "Transcript".to_owned(),
        description: None,
        domain_id: None,
        action: None,
        delivery: None,
        active: false,
        selected: false,
        focused,
        bounds: area,
    });
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
            let previous = index
                .checked_sub(1)
                .and_then(|previous| view.messages.get(previous));
            message_lines(
                view,
                index,
                message,
                MessageLayout {
                    focused,
                    mode,
                    unread: unread == Some(index),
                    width: area.width,
                    available_height: area.height,
                    grouped_with_previous: mode == ViewMode::Default
                        && previous.is_some_and(|previous| messages_group(previous, message)),
                    date_boundary: !message.details.date_label.is_empty()
                        && previous.is_none_or(|previous| {
                            previous.details.date_label != message.details.date_label
                        }),
                },
                graphics,
            )
        })
        .collect::<Vec<_>>();
    let heights = rendered_messages
        .iter()
        .map(|lines| u16::try_from(lines.len()).unwrap_or(u16::MAX))
        .collect::<Vec<_>>();
    let anchor = view
        .active_message
        .or(view.transcript_anchor)
        .unwrap_or_else(|| view.messages.len().saturating_sub(1));
    let range = transcript_window(&heights, Some(anchor), area.height);
    let message_area = bottom_aligned_area(area, range.clone(), &heights);
    render_semantics(
        message_area,
        view,
        focused,
        range.clone(),
        &heights,
        semantics,
    );
    let items = rendered_messages
        .into_iter()
        .enumerate()
        .skip(range.start)
        .take(range.len())
        .map(|(index, lines)| {
            let visible = usize::from(message_area.height);
            if index == anchor && lines.len() > visible {
                let hidden = lines.len() - visible;
                ListItem::new(lines.into_iter().skip(hidden).collect::<Vec<_>>())
            } else {
                ListItem::new(lines)
            }
        });
    frame.render_widget(List::new(items).style(surface_style(focused)), message_area);
}

fn bottom_aligned_area(area: Rect, range: std::ops::Range<usize>, heights: &[u16]) -> Rect {
    let content_height = heights[range]
        .iter()
        .copied()
        .fold(0_u16, u16::saturating_add)
        .min(area.height);
    Rect::new(
        area.x,
        area.bottom().saturating_sub(content_height),
        area.width,
        content_height,
    )
}

fn render_fresh_loading(frame: &mut Frame<'_>, area: Rect, view: &View, focused: bool) {
    let lines = fresh_loading_lines(area, view.animation_frame);
    let height = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .min(area.height);
    let centered = Rect::new(
        area.x,
        area.y
            .saturating_add(area.height.saturating_sub(height) / 2),
        area.width,
        height,
    );
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .style(surface_style(focused)),
        centered,
    );
}

fn fresh_loading_lines(area: Rect, frame: u8) -> Vec<Line<'static>> {
    if area.width < 40 {
        return vec![Line::from(effort_spans("loading", frame))];
    }

    let progress = loading_progress(frame);
    match area.height {
        0 => Vec::new(),
        1 => vec![progress],
        2 => vec![progress, Line::from(effort_spans("syncing chat", frame))],
        _ => vec![
            Line::styled(
                "INTUIGRAM",
                Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD),
            ),
            progress,
            Line::from(effort_spans("syncing chat", frame)),
        ],
    }
}

fn loading_progress(frame: u8) -> Line<'static> {
    const TRACK: usize = 12;
    let plane = usize::from(frame) % TRACK;
    let mut spans = Vec::with_capacity(TRACK + 2);
    spans.push(Span::styled("[", Style::default().fg(MUTED_TEXT)));
    for position in 0..TRACK {
        if position == plane {
            spans.push(Span::styled(
                ">",
                Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD),
            ));
        } else if position < plane {
            spans.push(Span::styled("-", Style::default().fg(PRIMARY)));
        } else {
            spans.push(Span::styled(".", Style::default().fg(MUTED_TEXT)));
        }
    }
    spans.push(Span::styled("]", Style::default().fg(MUTED_TEXT)));
    Line::from(spans)
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
            selected: view.selected_messages.contains(&message.id),
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
                selected: view.selected_messages.contains(&message.id),
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
