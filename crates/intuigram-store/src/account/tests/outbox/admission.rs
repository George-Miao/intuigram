use std::fs;

use refinery::Target;
use rusqlite::Connection;
use tempfile::tempdir;

use super::super::sync_batch;
use crate::{
    AccountDatabase, OutboxAdmission, OutboxExpiry, OutboxMedia, OutboxOperation, OutboxPayload,
    OutboxPayloadV1, OutboxPoll, OutboxState, StoreLayout, StoredDraft, StoredMessage,
};

#[test]
fn admission_atomically_consumes_the_scoped_draft_and_keeps_exact_media() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    database
        .commit_sync(sync_batch())
        .expect("owning Chat should persist");
    database
        .save_draft(StoredDraft {
            chat_id: 7,
            thread_root: Some(11),
            saved_peer: Some(13),
            text: "send me".to_owned(),
            reply_to: None,
            modified_at: 20,
        })
        .expect("scoped Draft should persist");
    let optimistic = message(-1, "send me");
    let media = OutboxMedia::new(
        "photo.png".to_owned(),
        "image/png".to_owned(),
        vec![0, 1, 2, 3, 255],
    );

    let id = database
        .admit_outbox(OutboxAdmission {
            operation: OutboxOperation::Send,
            payload: payload(-1, b"send me"),
            media: vec![media.clone()],
            optimistic_message: Some(optimistic.clone()),
            consume_draft: true,
            admitted_at: 30,
            expiry: OutboxExpiry::Never,
        })
        .expect("Outbox admission should commit");

    let cached = database
        .cached_account()
        .expect("durable cache should load");
    assert!(cached.drafts.is_empty());
    assert!(cached.messages.contains(&optimistic));
    assert_eq!(cached.outbox.len(), 1);
    assert_eq!(cached.outbox[0].id, id);
    assert_eq!(cached.outbox[0].state, OutboxState::Ready);
    assert_eq!(cached.outbox[0].media, vec![media]);

    assert!(matches!(
        database.claim_outbox(40).expect("claim should complete"),
        OutboxPoll::Claimed(record) if record.id == id
    ));
    let mut replacement = message(43, "send me");
    replacement.delivery = "sent".to_owned();
    replacement.timestamp = "12:01".to_owned();
    database
        .acknowledge_outbox(id, Some(replacement.clone()))
        .expect("acknowledgement and server replacement should commit");
    let acknowledged = database
        .cached_account()
        .expect("acknowledged cache should load");
    assert!(acknowledged.outbox.is_empty());
    assert!(!acknowledged.messages.iter().any(|message| message.id == -1));
    assert!(acknowledged.messages.contains(&replacement));
}

#[test]
fn failed_optimistic_message_write_rolls_back_admission_and_draft_consumption() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    let draft = StoredDraft {
        chat_id: 7,
        thread_root: Some(11),
        saved_peer: Some(13),
        text: "keep me".to_owned(),
        reply_to: None,
        modified_at: 20,
    };
    database
        .save_draft(draft.clone())
        .expect("Draft should persist without a cached Chat");
    let admission = OutboxAdmission {
        operation: OutboxOperation::Send,
        payload: payload(-1, b"will fail"),
        media: Vec::new(),
        optimistic_message: Some(message(-1, "will fail")),
        consume_draft: true,
        admitted_at: 30,
        expiry: OutboxExpiry::Never,
    };

    assert!(database.admit_outbox(admission).is_err());
    let cached = database
        .cached_account()
        .expect("rolled-back cache should load");
    assert_eq!(cached.drafts, vec![draft]);
    assert!(cached.messages.is_empty());
    assert!(cached.outbox.is_empty());
}

#[test]
fn failed_server_replacement_keeps_the_outbox_and_media_in_flight() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    let media = OutboxMedia::new(
        "photo.png".to_owned(),
        "image/png".to_owned(),
        vec![1, 2, 3, 4],
    );
    let id = database
        .admit_outbox(OutboxAdmission {
            operation: OutboxOperation::Send,
            payload: payload(-1, b"send me"),
            media: vec![media.clone()],
            optimistic_message: None,
            consume_draft: false,
            admitted_at: 30,
            expiry: OutboxExpiry::Never,
        })
        .expect("Outbox item should persist without a cached Chat");
    assert!(matches!(
        database.claim_outbox(40).expect("claim should complete"),
        OutboxPoll::Claimed(record) if record.id == id
    ));
    let replacement = message(43, "server result");

    assert!(database.acknowledge_outbox(id, Some(replacement)).is_err());
    let cached = database
        .cached_account()
        .expect("rolled-back acknowledgement should load");
    assert!(cached.messages.is_empty());
    assert_eq!(cached.outbox.len(), 1);
    assert_eq!(cached.outbox[0].state, OutboxState::InFlight);
    assert_eq!(cached.outbox[0].media, vec![media]);
}

#[test]
fn version_thirteen_accounts_gain_an_empty_outbox_without_losing_drafts() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    fs::create_dir_all(layout.data_directory()).expect("data directory should be created");
    let mut connection =
        Connection::open(layout.pending_database()).expect("fixture database should open");
    crate::account::migrations::migrations::runner()
        .set_target(Target::Version(13))
        .run(&mut connection)
        .expect("released version thirteen schema should install");
    connection
        .execute(
            "INSERT INTO drafts(chat_id, thread_root_message_id, saved_peer_id, text, \
             modified_at) VALUES (7, 11, 13, 'keep me', 20)",
            [],
        )
        .expect("version thirteen Draft should insert");
    drop(connection);

    let cached = AccountDatabase::begin_login(&layout)
        .expect("version thirteen database should migrate")
        .cached_account()
        .expect("migrated cache should load");
    assert!(cached.outbox.is_empty());
    assert_eq!(cached.drafts.len(), 1);
    assert_eq!(cached.drafts[0].text, "keep me");
}

fn payload(local_message_id: i64, content: &[u8]) -> OutboxPayload {
    OutboxPayload::V1(OutboxPayloadV1 {
        chat_id: 7,
        thread_root: Some(11),
        saved_peer: Some(13),
        local_message_id: Some(local_message_id),
        random_id: 17,
        content: content.to_vec(),
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
        thread_root: Some(11),
        saved_peer: Some(13),
        content_kind: "text".to_owned(),
        metadata: String::new(),
    }
}
