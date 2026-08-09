use intuigram_app::{OutboxItemView, OutboxStateView};
use time::OffsetDateTime;

use super::*;

pub(super) fn message_outbox<'a>(
    view: &'a View,
    message: &MessageView,
) -> Option<&'a OutboxItemView> {
    let chat = view.active_chat.and_then(|index| view.chats.get(index))?.id;
    view.outbox
        .iter()
        .find(|item| item.chat == chat && item.local_message == Some(message.id))
}

pub(super) fn append_message_lifecycle(
    spans: &mut Vec<Span<'static>>,
    item: &OutboxItemView,
    animation_frame: u8,
    max_width: usize,
) {
    let lifecycle = Lifecycle::from(item);
    let separator_width = usize::from(!spans.is_empty()) * 3;
    let base_width = Line::from(spans.clone()).width();
    let full_fits = base_width
        .saturating_add(separator_width)
        .saturating_add(Line::from(lifecycle.full.as_str()).width())
        <= max_width;
    let compact_fits = base_width
        .saturating_add(separator_width)
        .saturating_add(Line::from(lifecycle.compact).width())
        <= max_width;
    let label = if full_fits {
        lifecycle.full
    } else if compact_fits {
        lifecycle.compact.to_owned()
    } else {
        spans.clear();
        capped_text(lifecycle.compact, max_width)
    };
    if label.is_empty() {
        return;
    }
    if !spans.is_empty() {
        spans.push(Span::styled(" · ", Style::default().fg(MUTED_TEXT)));
    }
    if lifecycle.effort {
        spans.extend(effort_spans(&label, animation_frame));
    } else {
        spans.push(Span::styled(label, Style::default().fg(MUTED_TEXT)));
    }
}

pub(super) fn pending_status(view: &View, width: u16) -> Option<Vec<Span<'static>>> {
    let count = view
        .outbox
        .iter()
        .filter(|item| is_pending(item.state))
        .count();
    if count == 0 {
        return None;
    }
    let label = if width >= 50 {
        format!("outbox {count} pending")
    } else {
        format!("outbox {count}")
    };
    Some(effort_spans(&label, view.animation_frame.wrapping_add(3)))
}

pub(super) const fn is_pending(state: OutboxStateView) -> bool {
    matches!(
        state,
        OutboxStateView::Ready
            | OutboxStateView::InFlight
            | OutboxStateView::Deferred
            | OutboxStateView::CancelRequested
    )
}

struct Lifecycle {
    full: String,
    compact: &'static str,
    effort: bool,
}

impl From<&OutboxItemView> for Lifecycle {
    fn from(item: &OutboxItemView) -> Self {
        match item.state {
            OutboxStateView::Ready => Self::effort("queued", "queued"),
            OutboxStateView::InFlight => Self::effort("sending…", "sending"),
            OutboxStateView::Deferred => Self::effort(retry_label(item.available_at), "retry"),
            OutboxStateView::CancelRequested => Self::effort("cancelling…", "cancelling"),
            OutboxStateView::Failed => Self::terminal(failure_label(item), "failed"),
            OutboxStateView::Conflict => Self::terminal("conflict", "conflict"),
            OutboxStateView::OutcomeUnknown => Self::terminal("outcome unknown", "unknown"),
            OutboxStateView::Expired => Self::terminal("expired", "expired"),
            OutboxStateView::Cancelled => Self::terminal("cancelled", "cancelled"),
        }
    }
}

impl Lifecycle {
    fn effort(full: impl Into<String>, compact: &'static str) -> Self {
        Self {
            full: full.into(),
            compact,
            effort: true,
        }
    }

    fn terminal(full: impl Into<String>, compact: &'static str) -> Self {
        Self {
            full: full.into(),
            compact,
            effort: false,
        }
    }
}

fn retry_label(available_at: Option<i64>) -> String {
    let Some(available_at) = available_at else {
        return "retry pending".to_owned();
    };
    let Ok(time) = OffsetDateTime::from_unix_timestamp(available_at) else {
        return format!("retry at {available_at}");
    };
    format!(
        "retry {:04}-{:02}-{:02} {:02}:{:02}Z",
        time.year(),
        u8::from(time.month()),
        time.day(),
        time.hour(),
        time.minute(),
    )
}

fn failure_label(item: &OutboxItemView) -> String {
    let Some(reason) = item.last_error.as_deref() else {
        return "failed".to_owned();
    };
    let reason = reason.split_whitespace().collect::<Vec<_>>().join(" ");
    if reason.is_empty() {
        "failed".to_owned()
    } else {
        format!("failed: {reason}")
    }
}
