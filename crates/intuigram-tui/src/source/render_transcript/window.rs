use super::*;

pub(super) fn message_height(
    message: &MessageView,
    mode: ViewMode,
    preview: Option<&intuigram_app::InlineImage>,
    unread: bool,
) -> u16 {
    let body_height = u16::try_from(message.body.split('\n').count()).unwrap_or(u16::MAX);
    let media_height = message
        .details
        .media
        .as_ref()
        .map_or(0, |media| media_line_count(media, preview));
    mode.item_height(
        u16::from(unread)
            .saturating_add(1)
            .saturating_add(body_height)
            .saturating_add(media_height),
    )
}

pub(super) fn transcript_window(
    view: &View,
    active: Option<usize>,
    available: u16,
    mode: ViewMode,
    unread: Option<usize>,
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
        let height = height_at(view, start - 1, mode, unread);
        if before_height.saturating_add(height) > before_budget {
            break;
        }
        start -= 1;
        before_height = before_height.saturating_add(height);
    }
    let mut end = active + 1;
    let mut used = before_height.saturating_add(height_at(view, active, mode, unread));
    while end < messages.len() {
        let height = height_at(view, end, mode, unread);
        if used.saturating_add(height) > available {
            break;
        }
        used = used.saturating_add(height);
        end += 1;
    }
    while start > 0 {
        let height = height_at(view, start - 1, mode, unread);
        if used.saturating_add(height) > available {
            break;
        }
        used = used.saturating_add(height);
        start -= 1;
    }
    start..end
}

pub(super) fn unread_boundary_index(view: &View) -> Option<usize> {
    let boundary = view.unread_boundary?;
    view.messages
        .iter()
        .position(|message| message.id == boundary)
}

fn height_at(view: &View, index: usize, mode: ViewMode, unread: Option<usize>) -> u16 {
    let message = &view.messages[index];
    message_height(
        message,
        mode,
        media_preview(view, message.id),
        unread == Some(index),
    )
}
