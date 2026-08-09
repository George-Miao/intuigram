use std::fs;

use refinery::Target;
use rusqlite::Connection;
use tempfile::tempdir;

use super::{
    AccountDatabase, StoredChat, StoredDraft, StoredFolder, StoredMessage, StoredSelection,
    SyncBatch, SyncCursor,
};
use crate::{AccountId, StoreLayout};

mod lifecycle;
mod offline_media;
mod pinned;
mod saved_dialogs;
mod topics;

#[test]
fn version_three_chats_receive_safe_pin_permission_defaults() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    fs::create_dir_all(layout.data_directory()).expect("data directory should be created");
    let mut connection =
        Connection::open(layout.pending_database()).expect("fixture database should open");
    super::migrations::migrations::runner()
        .set_target(Target::Version(3))
        .run(&mut connection)
        .expect("released version three schema should install");
    connection
        .execute(
            "INSERT INTO chats(chat_id, kind, title, preview, unread_count, pinned) VALUES (1, \
             'private', 'Ada', '', 0, 0), (2, 'supergroup', 'Rust', '', 0, 0)",
            [],
        )
        .expect("version three Chat fixtures should insert");
    drop(connection);

    let database = AccountDatabase::begin_login(&layout)
        .expect("version three database should migrate to the current schema");
    let chats = database
        .cached_account()
        .expect("migrated Chats should load")
        .chats;

    assert!(
        chats
            .iter()
            .find(|chat| chat.id == 1)
            .expect("private Chat fixture should remain")
            .can_pin_messages
    );
    assert!(
        !chats
            .iter()
            .find(|chat| chat.id == 2)
            .expect("supergroup Chat fixture should remain")
            .can_pin_messages
    );
}

#[test]
fn version_four_chats_receive_an_empty_status_without_losing_records() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    fs::create_dir_all(layout.data_directory()).expect("data directory should be created");
    let mut connection =
        Connection::open(layout.pending_database()).expect("fixture database should open");
    super::migrations::migrations::runner()
        .set_target(Target::Version(4))
        .run(&mut connection)
        .expect("released version four schema should install");
    connection
        .execute(
            "INSERT INTO chats(chat_id, kind, title, preview, unread_count, pinned, \
             can_pin_messages) VALUES (1, 'private', 'Ada', '', 0, 0, 1)",
            [],
        )
        .expect("version four Chat fixture should insert");
    drop(connection);

    let database = AccountDatabase::begin_login(&layout)
        .expect("version four database should migrate to the current schema");
    let chats = database
        .cached_account()
        .expect("migrated Chats should load")
        .chats;

    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].title, "Ada");
    assert!(chats[0].status.is_empty());
}

#[test]
fn version_six_selection_gains_an_empty_transcript_anchor() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    fs::create_dir_all(layout.data_directory()).expect("data directory should be created");
    let mut connection =
        Connection::open(layout.pending_database()).expect("fixture database should open");
    super::migrations::migrations::runner()
        .set_target(Target::Version(6))
        .run(&mut connection)
        .expect("released version six schema should install");
    connection
        .execute(
            "INSERT INTO ui_selection(singleton, folder_id, chat_id) VALUES (1, 0, 7)",
            [],
        )
        .expect("version six selection should insert");
    drop(connection);

    let selection = AccountDatabase::begin_login(&layout)
        .expect("version six database should migrate")
        .cached_account()
        .expect("migrated selection should load")
        .selection
        .expect("selection should remain");

    assert_eq!(selection.folder_id, 0);
    assert_eq!(selection.chat_id, Some(7));
    assert_eq!(selection.anchor_message_id, None);
}

#[test]
fn version_eight_chats_gain_an_empty_authoritative_position() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    fs::create_dir_all(layout.data_directory()).expect("data directory should be created");
    let mut connection =
        Connection::open(layout.pending_database()).expect("fixture database should open");
    super::migrations::migrations::runner()
        .set_target(Target::Version(8))
        .run(&mut connection)
        .expect("released version eight schema should install");
    connection
        .execute(
            "INSERT INTO chats(chat_id, kind, title, preview, status, unread_count, pinned, \
             can_pin_messages) VALUES (1, 'private', 'Ada', '', '', 0, 0, 1)",
            [],
        )
        .expect("version eight Chat fixture should insert");
    drop(connection);

    let chats = AccountDatabase::begin_login(&layout)
        .expect("version eight database should migrate")
        .cached_account()
        .expect("migrated cache should load")
        .chats;

    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].title, "Ada");
}

#[test]
fn normalized_records_and_cursor_commit_or_roll_back_together() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    let mut invalid = sync_batch();
    invalid.messages[0].chat_id = 999;

    assert!(database.commit_sync(invalid).is_err());
    assert_eq!(
        database
            .cached_account()
            .expect("rolled-back cache should load"),
        super::CachedAccount::default()
    );

    database
        .commit_sync(sync_batch())
        .expect("valid synchronized records should commit");
    let cached = database
        .cached_account()
        .expect("committed cache should load");
    assert_eq!(cached.cursors, sync_batch().cursors);
    assert_eq!(cached.folders, sync_batch().folders);
    assert_eq!(cached.chats, sync_batch().chats);
    assert_eq!(cached.messages, sync_batch().messages);
}

#[test]
fn authoritative_bootstrap_order_is_restored_from_the_cache() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    let mut batch = sync_batch();
    let mut second = batch.chats[0].clone();
    second.id = 8;
    second.title = "Rust".to_owned();
    batch.chats.push(second);
    batch.chat_order = Some(vec![8, 7]);

    database
        .commit_sync(batch)
        .expect("ordered Chat projection should commit");

    assert_eq!(
        database
            .cached_account()
            .expect("ordered Chat cache should load")
            .chats
            .iter()
            .map(|chat| chat.id)
            .collect::<Vec<_>>(),
        vec![8, 7]
    );
}

#[test]
fn replacing_a_draft_keeps_the_current_value_durable() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    database
        .save_draft(StoredDraft {
            chat_id: 7,
            thread_root: None,
            text: "first".to_owned(),
            reply_to: None,
            modified_at: 10,
        })
        .expect("initial Draft should persist");
    let replacement = StoredDraft {
        chat_id: 7,
        thread_root: None,
        text: "second".to_owned(),
        reply_to: Some(3),
        modified_at: 20,
    };

    database
        .save_draft(replacement.clone())
        .expect("replacement Draft should persist");

    assert_eq!(
        database
            .cached_account()
            .expect("Draft cache should load")
            .drafts,
        vec![replacement]
    );
}

#[test]
fn replacing_the_ui_selection_keeps_the_current_value_durable() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let account = AccountId::new(7).expect("fixture account ID should be valid");
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    database
        .save_selection(StoredSelection {
            folder_id: 0,
            chat_id: Some(7),
            anchor_message_id: Some(6),
            transcript_anchors: Vec::new(),
        })
        .expect("initial selection should persist");
    let replacement = StoredSelection {
        folder_id: -1,
        chat_id: Some(9),
        anchor_message_id: Some(8),
        transcript_anchors: vec![
            super::StoredTranscriptAnchor {
                chat_id: 9,
                thread_root: None,
                saved_peer: None,
                message_id: 8,
            },
            super::StoredTranscriptAnchor {
                chat_id: 10,
                thread_root: Some(3),
                saved_peer: None,
                message_id: 4,
            },
        ],
    };

    database
        .save_selection(replacement.clone())
        .expect("replacement selection should persist");
    let database = database
        .finish_login(&layout, account)
        .expect("selection database should be promoted");
    drop(database);
    let reopened =
        AccountDatabase::open(&layout, account).expect("selection database should reopen");

    assert_eq!(
        reopened
            .cached_account()
            .expect("selection cache should load")
            .selection,
        Some(replacement)
    );
}

pub(super) fn sync_batch() -> SyncBatch {
    SyncBatch {
        cursors: vec![SyncCursor {
            scope: "account".to_owned(),
            pts: 12,
            qts: 0,
            date: 34,
            seq: 5,
        }],
        folders: vec![StoredFolder {
            id: 0,
            title: "All".to_owned(),
            unread: 1,
        }],
        chats: vec![StoredChat {
            id: 7,
            kind: "private".to_owned(),
            title: "Ada".to_owned(),
            preview: "hello".to_owned(),
            status: "online".to_owned(),
            unread: 1,
            pinned: false,
            can_pin_messages: true,
            has_topics: false,
            folders: vec![0],
        }],
        chat_order: Some(vec![7]),
        messages: vec![StoredMessage {
            chat_id: 7,
            id: 42,
            sender: "Ada".to_owned(),
            body: "hello".to_owned(),
            timestamp: "12:00".to_owned(),
            direction: "incoming".to_owned(),
            delivery: "sent".to_owned(),
            reply_to: None,
            thread_root: Some(41),
            saved_peer: None,
            content_kind: "text".to_owned(),
            metadata: String::new(),
        }],
        mutations: Vec::new(),
    }
}
