use super::*;

#[test]
fn passive_short_message_is_normalized_at_the_serialized_tl_boundary() {
    let update = tl::enums::Updates::UpdateShortMessage(tl::types::UpdateShortMessage {
        out: false,
        mentioned: false,
        media_unread: false,
        silent: false,
        id: 42,
        user_id: 7,
        message: "hello".to_owned(),
        pts: 9,
        pts_count: 1,
        date: 1_700_000_000,
        fwd_from: None,
        via_bot_id: None,
        reply_to: None,
        entities: None,
        ttl_period: None,
    });
    let mut names = [(ChatId(7), "Ada".to_owned())].into_iter().collect();

    let batch = normalize_live_update(&update.to_bytes(), &mut names)
        .expect("serialized short update should normalize");

    assert_eq!(batch.cursors[0].pts, Some(9));
    assert_eq!(batch.cursors[0].date, Some(1_700_000_000));
    assert_eq!(batch.events.len(), 1);
    let AdapterEvent::MessageAdded { chat, message } = &batch.events[0] else {
        panic!("short message should produce a message event")
    };
    assert_eq!(*chat, ChatId(7));
    assert_eq!(message.id, MessageId(42));
    assert_eq!(message.sender, "Ada");
    assert_eq!(message.body, "hello");
    assert_eq!(message.direction, MessageDirection::Incoming);
    assert_eq!(message.details.sender_peer, Some(ChatId(7)));
    assert!(!message.details.date_label.is_empty());
}

#[test]
fn affected_messages_rpc_result_advances_account_cursor() {
    let affected =
        tl::enums::messages::AffectedMessages::Messages(tl::types::messages::AffectedMessages {
            pts: 41,
            pts_count: 2,
        });

    let batch = normalize_live_update(&affected.to_bytes(), &mut HashMap::new())
        .expect("affected Messages should normalize as an own update");

    assert!(batch.events.is_empty());
    assert_eq!(
        batch.cursors,
        vec![crate::UpdateCursor {
            scope: UpdateScope::Account,
            pts: Some(41),
            pts_count: 2,
            ..crate::UpdateCursor::default()
        }]
    );
}

#[test]
fn affected_messages_channel_request_uses_channel_cursor() {
    let affected =
        tl::enums::messages::AffectedMessages::Messages(tl::types::messages::AffectedMessages {
            pts: 43,
            pts_count: 1,
        });
    let request = tl::functions::channels::DeleteMessages {
        channel: tl::types::InputChannel {
            channel_id: 73,
            access_hash: 91,
        }
        .into(),
        id: vec![5],
    };

    let batch = normalize_correlated_update(
        &affected.to_bytes(),
        Some(&request.to_bytes()),
        &mut HashMap::new(),
    )
    .expect("channel mutation cursor should normalize");

    assert!(batch.events.is_empty());
    assert_eq!(
        batch.cursors,
        vec![crate::UpdateCursor {
            scope: UpdateScope::Channel(ChatId(crate::mark_channel_id(73))),
            pts: Some(43),
            pts_count: 1,
            ..crate::UpdateCursor::default()
        }]
    );
}
