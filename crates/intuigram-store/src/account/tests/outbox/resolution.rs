use tempfile::tempdir;

use super::super::sync_batch;
use crate::{
    AccountDatabase, OutboxAdmission, OutboxExpiry, OutboxOperation, OutboxPayload,
    OutboxPayloadV1, OutboxPoll, OutboxState, StoreLayout, StoredMessage,
};

#[test]
fn only_replay_safe_failed_work_uses_the_ordinary_retry_endpoint() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    let send = database
        .admit_outbox(admission(OutboxOperation::Send, 10, None))
        .expect("send should be admitted");
    claim(&database, send);
    database
        .fail_outbox(send, "offline".to_owned())
        .expect("send should fail");

    database
        .retry_outbox(send)
        .expect("replay-safe send should return to Ready");
    assert_eq!(record(&database, send).state, OutboxState::Ready);
    database
        .cancel_outbox(send)
        .expect("retried send should remain cancellable");

    let mutation = database
        .admit_outbox(admission(OutboxOperation::Mutation, 20, None))
        .expect("mutation should be admitted");
    claim(&database, mutation);
    database
        .fail_outbox(mutation, "rejected".to_owned())
        .expect("mutation should fail");
    assert!(database.retry_outbox(mutation).is_err());
    assert_eq!(record(&database, mutation).state, OutboxState::Failed);
}

#[test]
fn resolving_a_conflict_replaces_its_versioned_basis_before_retry() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    let id = database
        .admit_outbox(admission(OutboxOperation::Mutation, 10, None))
        .expect("mutation should be admitted");
    claim(&database, id);
    database
        .conflict_outbox(id, "stale basis".to_owned())
        .expect("mutation should conflict");
    let replacement = payload(99, Some(42));

    database
        .resolve_outbox_conflict(id, replacement.clone())
        .expect("replacement basis should resolve the conflict");

    let resolved = record(&database, id);
    assert_eq!(resolved.state, OutboxState::Ready);
    assert_eq!(resolved.payload, replacement);
    assert_eq!(resolved.last_error, None);
}

#[test]
fn unknown_outcome_requires_its_explicit_user_resolution_endpoint() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    let id = database
        .admit_outbox(admission(OutboxOperation::Mutation, 10, None))
        .expect("mutation should be admitted");
    claim(&database, id);
    database
        .mark_outbox_outcome_unknown(id, "response lost".to_owned())
        .expect("outcome should become unknown");

    assert!(database.retry_outbox(id).is_err());
    database
        .resolve_outbox_outcome_unknown(id)
        .expect("explicit user resolution should permit retry");

    assert_eq!(record(&database, id).state, OutboxState::Ready);
}

#[test]
fn dismissing_a_terminal_record_does_not_delete_messages() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    database
        .commit_sync(sync_batch())
        .expect("unrelated synchronized Message should persist");
    let optimistic = message(-1, "still visible");
    let id = database
        .admit_outbox(admission(
            OutboxOperation::Send,
            10,
            Some(optimistic.clone()),
        ))
        .expect("send should be admitted");
    claim(&database, id);
    database
        .fail_outbox(id, "permanent".to_owned())
        .expect("send should fail");

    database
        .dismiss_outbox(id)
        .expect("terminal record should be dismissed");

    let cached = database.cached_account().expect("cache should load");
    assert!(cached.outbox.is_empty());
    assert!(cached.messages.contains(&optimistic));
    assert!(cached.messages.iter().any(|message| message.id == 42));
}

fn admission(
    operation: OutboxOperation,
    admitted_at: i64,
    optimistic_message: Option<StoredMessage>,
) -> OutboxAdmission {
    OutboxAdmission {
        operation,
        payload: payload(
            admitted_at,
            optimistic_message.as_ref().map(|message| message.id),
        ),
        media: Vec::new(),
        optimistic_message,
        consume_draft: false,
        admitted_at,
        expiry: OutboxExpiry::Never,
    }
}

fn payload(random_id: i64, local_message_id: Option<i64>) -> OutboxPayload {
    OutboxPayload::V1(OutboxPayloadV1 {
        chat_id: 7,
        thread_root: None,
        saved_peer: None,
        local_message_id,
        random_id,
        content: random_id.to_le_bytes().to_vec(),
    })
}

fn message(id: i64, body: &str) -> StoredMessage {
    StoredMessage {
        chat_id: 7,
        id,
        sender: "You".to_owned(),
        body: body.to_owned(),
        timestamp: "now".to_owned(),
        direction: "outgoing".to_owned(),
        delivery: "sending".to_owned(),
        reply_to: None,
        thread_root: None,
        saved_peer: None,
        content_kind: "text".to_owned(),
        metadata: String::new(),
    }
}

fn claim(database: &AccountDatabase, expected: crate::OutboxId) {
    assert!(matches!(
        database.claim_outbox(30).expect("claim should complete"),
        OutboxPoll::Claimed(record) if record.id == expected
    ));
}

fn record(database: &AccountDatabase, id: crate::OutboxId) -> crate::OutboxRecord {
    database
        .load_outbox()
        .expect("Outbox should load")
        .into_iter()
        .find(|record| record.id == id)
        .expect("Outbox item should remain")
}
