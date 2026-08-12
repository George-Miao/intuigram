use super::*;

pub(super) fn transcript_window(
    heights: &[u16],
    active: Option<usize>,
    available: u16,
) -> std::ops::Range<usize> {
    if heights.is_empty() {
        return 0..0;
    }
    let active = active.unwrap_or(heights.len() - 1).min(heights.len() - 1);
    let mut start = active;
    let mut before_height = 0_u16;
    let before_budget = available / 3;
    while start > 0 {
        let height = heights[start - 1];
        if before_height.saturating_add(height) > before_budget {
            break;
        }
        start -= 1;
        before_height = before_height.saturating_add(height);
    }
    let mut end = active + 1;
    let mut used = before_height.saturating_add(heights[active]);
    while end < heights.len() {
        let height = heights[end];
        if used.saturating_add(height) > available {
            break;
        }
        used = used.saturating_add(height);
        end += 1;
    }
    while start > 0 {
        let height = heights[start - 1];
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
