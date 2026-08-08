//! History reconciliation independent of UI state transitions.

use std::cmp::Ordering;
use std::collections::HashSet;

use crate::{DeliveryState, MessageId, MessageView};

/// Merges an authoritative refresh with cached entries that the request may
/// not have observed, such as concurrent live updates and optimistic sends.
pub(crate) fn reconcile_refresh(
    cached: Option<&[MessageView]>,
    refreshed: Vec<MessageView>,
    request_baseline: Option<&HashSet<MessageId>>,
) -> Vec<MessageView> {
    let oldest_refreshed = refreshed
        .iter()
        .filter(|message| !message.id.0.is_negative())
        .map(|message| message.id)
        .min();
    let mut merged: Vec<MessageView> = Vec::with_capacity(
        refreshed
            .len()
            .saturating_add(cached.map_or(0, <[MessageView]>::len)),
    );
    for message in refreshed {
        if let Some(existing) = merged
            .iter_mut()
            .find(|candidate| candidate.id == message.id)
        {
            existing.clone_from(&message);
        } else {
            merged.push(message);
        }
    }
    if let Some(cached) = cached {
        for message in cached {
            if !merged.iter().any(|candidate| candidate.id == message.id)
                && should_retain_cached(message, request_baseline, oldest_refreshed)
            {
                merged.push(message.clone());
            }
        }
    }
    merged.sort_by(message_order);
    merged
}

fn should_retain_cached(
    message: &MessageView,
    request_baseline: Option<&HashSet<MessageId>>,
    oldest_refreshed: Option<MessageId>,
) -> bool {
    matches!(
        message.delivery,
        DeliveryState::Pending | DeliveryState::Failed
    ) || message.id.0.is_negative()
        || request_baseline.is_none_or(|baseline| !baseline.contains(&message.id))
        || oldest_refreshed.is_some_and(|oldest| message.id < oldest)
}

fn message_order(left: &MessageView, right: &MessageView) -> Ordering {
    match (left.id.0.is_negative(), right.id.0.is_negative()) {
        (false, false) => left.id.cmp(&right.id),
        (false, true) => Ordering::Less,
        (true, false) => Ordering::Greater,
        (true, true) => right.id.cmp(&left.id),
    }
}

#[cfg(test)]
mod tests {
    use super::reconcile_refresh;
    use crate::{DeliveryState, MessageDetails, MessageDirection, MessageId, MessageView};

    fn message(id: i64, body: &str) -> MessageView {
        MessageView {
            id: MessageId(id),
            sender: "Lin".to_owned(),
            body: body.to_owned(),
            timestamp: String::new(),
            direction: MessageDirection::Incoming,
            delivery: DeliveryState::Read,
            reply_to: None,
            details: MessageDetails::default(),
        }
    }

    #[test]
    fn refresh_keeps_unobserved_live_and_pending_messages_in_order() {
        let cached = vec![
            message(1, "old"),
            message(3, "live"),
            message(-1, "first pending"),
            message(-2, "second pending"),
        ];

        let baseline = [MessageId(1), MessageId(-1), MessageId(-2)]
            .into_iter()
            .collect();
        let merged = reconcile_refresh(
            Some(&cached),
            vec![message(1, "refreshed"), message(2, "new")],
            Some(&baseline),
        );

        assert_eq!(
            merged
                .iter()
                .map(|message| (message.id.0, message.body.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (1, "refreshed"),
                (2, "new"),
                (3, "live"),
                (-1, "first pending"),
                (-2, "second pending"),
            ]
        );
    }

    #[test]
    fn refresh_prunes_acknowledged_baseline_messages_from_the_refreshed_window() {
        let cached = vec![
            message(1, "older"),
            message(8, "deleted"),
            message(11, "concurrent live"),
        ];
        let baseline = [MessageId(1), MessageId(8)].into_iter().collect();

        let merged = reconcile_refresh(
            Some(&cached),
            vec![message(7, "fresh"), message(10, "latest")],
            Some(&baseline),
        );

        assert_eq!(
            merged
                .iter()
                .map(|message| message.id.0)
                .collect::<Vec<_>>(),
            vec![1, 7, 10, 11]
        );
    }
}
