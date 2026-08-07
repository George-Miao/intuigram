use rusqlite::Connection;
use tempfile::tempdir;

use crate::{AccountDatabase, AccountId, AccountOpen, SessionMaterial, StoreLayout, StoredDraft};

#[test]
fn rebuild_preserves_unique_records_and_the_broken_database() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let account = AccountId::new(7).expect("fixture ID should be positive");
    let session = SessionMaterial::new(2, "149.154.167.40:443".to_owned(), [0xa5; 256], -2, 42);
    let draft = StoredDraft {
        chat_id: 9,
        thread_root: None,
        text: "keep this".to_owned(),
        reply_to: Some(3),
        modified_at: 10,
    };
    let pending = AccountDatabase::begin_login(&layout).expect("pending database should open");
    pending
        .save_session(session.clone())
        .expect("session should persist");
    pending
        .save_draft(draft.clone())
        .expect("Draft should persist");
    let database = pending
        .finish_login(&layout, account)
        .expect("database should be promoted");
    drop(database);

    let path = layout.account_database(account);
    let connection = Connection::open(&path).expect("fixture database should open");
    connection
        .execute_batch(
            "PRAGMA foreign_keys = OFF;
             INSERT INTO chat_folders (chat_id, folder_id, position) VALUES (999, 0, 0);",
        )
        .expect("fixture should introduce a foreign-key violation");
    drop(connection);

    let AccountOpen::Recovery(recovery) =
        AccountDatabase::open_recoverable(&layout, account).expect("recovery should be described")
    else {
        panic!("corrupt synchronized cache must not open normally");
    };
    assert_eq!(recovery.database_path(), path);
    assert!(recovery.can_rebuild_cache());

    let rebuilt = recovery
        .rebuild_cache()
        .expect("verified unique records should rebuild safely");
    assert!(rebuilt.preserved_original().is_file());
    assert_ne!(rebuilt.preserved_original(), path);
    assert_eq!(
        rebuilt.database().session().expect("session should load"),
        Some(session)
    );
    let cached = rebuilt
        .database()
        .cached_account()
        .expect("rebuilt cache should load");
    assert_eq!(cached.drafts, vec![draft]);
    assert!(cached.chats.is_empty());
    assert!(cached.messages.is_empty());
}

#[test]
fn rebuild_is_unavailable_when_unique_records_cannot_be_proven_readable() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let account = AccountId::new(8).expect("fixture ID should be positive");
    std::fs::create_dir_all(layout.data_directory()).expect("data directory should exist");
    let path = layout.account_database(account);
    std::fs::write(&path, b"not sqlite").expect("broken fixture should be written");

    let AccountOpen::Recovery(recovery) =
        AccountDatabase::open_recoverable(&layout, account).expect("recovery should be described")
    else {
        panic!("invalid database must not open normally");
    };
    assert!(!recovery.can_rebuild_cache());
    assert!(recovery.rebuild_blocker().is_some());
    assert_eq!(
        std::fs::read(path).expect("original should remain"),
        b"not sqlite"
    );
}
