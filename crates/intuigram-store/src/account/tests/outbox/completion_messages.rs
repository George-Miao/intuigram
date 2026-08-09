use tempfile::tempdir;

use super::super::sync_batch;
use crate::{
    AccountDatabase, OutboxAdmission, OutboxCompletion, OutboxExpiry, OutboxOperation,
    OutboxPayload, OutboxPayloadV1, OutboxPoll, StoreLayout, StoredMessage,
};

#[test]
fn send_and_create_completion_atomically_commit_their_normalized_message() {
    for operation in [OutboxOperation::Send, OutboxOperation::Create] {
        let temporary = tempdir().expect("temporary directory should be created");
        let layout = StoreLayout::new(temporary.path().join("intuigram"));
        let database =
            AccountDatabase::begin_login(&layout).expect("pending login database should open");
        database
            .commit_sync(sync_batch())
            .expect("unrelated synchronized Message should persist");
        let local_id = (operation == OutboxOperation::Send).then_some(-1);
        let optimistic = local_id.map(|id| message(id, "pending", "sending"));
        let id = database
            .admit_outbox(admission(operation, local_id, optimistic))
            .expect("item should be admitted");
        claim(&database, id);
        let server = message(50, "accepted", "sent");

        database
            .complete_outbox(id, OutboxCompletion::Message(Box::new(server.clone())))
            .expect("normalized completion should commit");

        let cached = database.cached_account().expect("cache should load");
        assert!(cached.outbox.is_empty());
        assert!(cached.messages.contains(&server));
        assert!(cached.messages.iter().any(|message| message.id == 42));
        assert!(!cached.messages.iter().any(|message| message.id == -1));
    }
}

#[test]
fn completion_failure_keeps_the_outbox_and_local_message_atomic() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    database
        .commit_sync(sync_batch())
        .expect("owning Chat should persist");
    let local = message(-1, "pending", "sending");
    let id = database
        .admit_outbox(admission(
            OutboxOperation::Send,
            Some(-1),
            Some(local.clone()),
        ))
        .expect("item should be admitted");
    claim(&database, id);
    let mut server = message(50, "accepted", "sent");
    server.chat_id = 8;

    assert!(
        database
            .complete_outbox(id, OutboxCompletion::Message(Box::new(server)))
            .is_err()
    );

    let cached = database.cached_account().expect("cache should load");
    assert_eq!(cached.outbox.len(), 1);
    assert!(cached.messages.contains(&local));
}

fn admission(
    operation: OutboxOperation,
    local_message_id: Option<i64>,
    optimistic_message: Option<StoredMessage>,
) -> OutboxAdmission {
    OutboxAdmission {
        operation,
        payload: OutboxPayload::V1(OutboxPayloadV1 {
            chat_id: 7,
            thread_root: None,
            saved_peer: None,
            local_message_id,
            random_id: 17,
            content: b"operation".to_vec(),
        }),
        media: Vec::new(),
        optimistic_message,
        consume_draft: false,
        admitted_at: 10,
        expiry: OutboxExpiry::Never,
    }
}

fn message(id: i64, body: &str, delivery: &str) -> StoredMessage {
    StoredMessage {
        chat_id: 7,
        id,
        sender: "You".to_owned(),
        body: body.to_owned(),
        timestamp: "now".to_owned(),
        direction: "outgoing".to_owned(),
        delivery: delivery.to_owned(),
        reply_to: None,
        thread_root: None,
        saved_peer: None,
        content_kind: "text".to_owned(),
        metadata: String::new(),
    }
}

fn claim(database: &AccountDatabase, expected: crate::OutboxId) {
    assert!(matches!(
        database.claim_outbox(20).expect("claim should complete"),
        OutboxPoll::Claimed(record) if record.id == expected
    ));
}
