use std::fs;

use refinery::Target;
use rusqlite::Connection;
use tempfile::tempdir;

use super::{
    AccountCipher, AccountDatabase, SessionMaterial, StoredChat, StoredDraft, StoredFolder,
    StoredMessage, StoredSelection, SyncBatch, SyncCursor, enable_local_lock,
};
use crate::{AccountId, StoreLayout};

mod pinned;

#[test]
fn local_lock_encrypts_new_and_existing_account_records() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let first = AccountId::new(41).expect("fixture ID should be positive");
    let second = AccountId::new(42).expect("fixture ID should be positive");
    let cipher = AccountCipher::encrypted([7; 32]);

    let encrypted = AccountDatabase::begin_login_with_cipher(&layout, cipher.clone())
        .expect("encrypted pending database should open")
        .finish_login(&layout, first)
        .expect("encrypted Account should promote");
    encrypted
        .save_draft(StoredDraft {
            chat_id: 9,
            thread_root: None,
            text: "private draft".to_owned(),
            reply_to: None,
            modified_at: 1,
        })
        .expect("encrypted draft should save");
    drop(encrypted);
    assert_ne!(
        &fs::read(layout.account_database(first)).expect("database should be readable")[..16],
        b"SQLite format 3\0"
    );
    assert!(AccountDatabase::open(&layout, first).is_err());
    assert_eq!(
        AccountDatabase::open_with_cipher(&layout, first, cipher.clone())
            .expect("correct key should open")
            .cached_account()
            .expect("encrypted cache should load")
            .drafts[0]
            .text,
        "private draft"
    );

    let plaintext = AccountDatabase::begin_login(&layout)
        .expect("plaintext pending database should open")
        .finish_login(&layout, second)
        .expect("plaintext Account should promote");
    plaintext
        .commit_sync(sync_batch())
        .expect("plaintext cache should save");
    drop(plaintext);
    enable_local_lock(&layout, second, &cipher).expect("existing Account should encrypt");
    let reopened = AccountDatabase::open_with_cipher(&layout, second, cipher)
        .expect("migrated Account should open");
    assert_eq!(
        reopened
            .cached_account()
            .expect("migrated cache should load")
            .messages[0]
            .body,
        "hello"
    );
}

#[test]
fn pending_login_is_promoted_to_a_persistent_account_database() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let account = AccountId::new(4_242).expect("fixture ID should be positive");

    let pending =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    assert_eq!(pending.account_id().expect("identity should be read"), None);

    let authorized = pending
        .finish_login(&layout, account)
        .expect("pending database should be promoted");
    assert_eq!(
        authorized.account_id().expect("identity should be read"),
        Some(account)
    );
    drop(authorized);

    let reopened =
        AccountDatabase::open(&layout, account).expect("promoted account database should reopen");
    assert_eq!(
        reopened.account_id().expect("identity should persist"),
        Some(account)
    );
    assert!(!layout.pending_database().exists());
    assert!(layout.account_database(account).exists());
}

#[test]
fn mtproto_session_round_trips_without_appearing_in_debug_output() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    let session = SessionMaterial::new(2, "149.154.167.40:443".to_owned(), [0xa5; 256], -2, 42);

    database
        .save_session(session.clone())
        .expect("session should persist");

    assert_eq!(
        database.session().expect("session should load"),
        Some(session.clone())
    );
    assert!(!format!("{session:?}").contains("a5"));
    assert!(format!("{session:?}").contains("[REDACTED]"));
}

#[test]
fn opening_a_missing_account_does_not_create_a_database() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let account = AccountId::new(7).expect("fixture ID should be positive");

    assert!(AccountDatabase::open(&layout, account).is_err());
    assert!(!layout.account_database(account).exists());
}

#[test]
fn promotion_never_replaces_an_existing_account_database() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    fs::create_dir_all(layout.data_directory()).expect("data directory should be created");
    let account = AccountId::new(8).expect("fixture ID should be positive");
    let target = layout.account_database(account);
    fs::write(&target, b"existing account").expect("existing account fixture should be written");
    let pending =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");

    assert!(pending.finish_login(&layout, account).is_err());
    assert_eq!(
        fs::read(target).expect("existing account fixture should remain"),
        b"existing account"
    );
    assert!(layout.pending_database().exists());
}

#[test]
fn an_existing_unmigrated_database_is_backed_up_before_migration() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    fs::create_dir_all(layout.data_directory()).expect("data directory should be created");
    let pending_path = layout.pending_database();
    let connection = Connection::open(&pending_path).expect("fixture database should open");
    connection
        .execute("CREATE TABLE legacy(value TEXT NOT NULL)", [])
        .expect("legacy schema should be created");
    drop(connection);

    let database =
        AccountDatabase::begin_login(&layout).expect("legacy database should migrate safely");
    drop(database);

    let backups = fs::read_dir(layout.data_directory())
        .expect("data directory should be readable")
        .filter_map(std::result::Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .contains("pre-migration")
        })
        .count();
    assert_eq!(backups, 1);
}

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
        })
        .expect("initial selection should persist");
    let replacement = StoredSelection {
        folder_id: -1,
        chat_id: Some(9),
        anchor_message_id: Some(8),
    };

    database
        .save_selection(replacement)
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

fn sync_batch() -> SyncBatch {
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
            folders: vec![0],
        }],
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
            content_kind: "text".to_owned(),
            metadata: String::new(),
        }],
        mutations: Vec::new(),
    }
}
