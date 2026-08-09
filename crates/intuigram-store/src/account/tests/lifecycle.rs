use std::fs;

use rusqlite::Connection;
use tempfile::tempdir;

use super::sync_batch;
use crate::account::{
    AccountCipher, AccountDatabase, SessionMaterial, StoredDraft, enable_local_lock,
};
use crate::{AccountId, StoreLayout};

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
            saved_peer: None,
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
fn local_lock_finishes_removing_plaintext_after_an_interrupted_install() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let account = AccountId::new(43).expect("fixture ID should be positive");
    let cipher = AccountCipher::encrypted([8; 32]);
    let database = AccountDatabase::begin_login(&layout)
        .expect("plaintext pending database should open")
        .finish_login(&layout, account)
        .expect("plaintext Account should promote");
    drop(database);
    let database_path = layout.account_database(account);
    let plaintext = database_path.with_extension("local-lock-plaintext.tmp");

    enable_local_lock(&layout, account, &cipher).expect("Account should encrypt");
    fs::write(&plaintext, b"sensitive plaintext workspace")
        .expect("interruption fixture should be written");

    enable_local_lock(&layout, account, &cipher).expect("cleanup should resume");

    assert!(!plaintext.exists());
    AccountDatabase::open_with_cipher(&layout, account, cipher)
        .expect("encrypted Account should remain readable");
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
        authorized.account_id().expect("identity should persist"),
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
