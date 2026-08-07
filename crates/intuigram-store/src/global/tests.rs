use std::fs;

use rusqlite::Connection;
use tempfile::tempdir;

use super::{AccountRecord, GlobalDatabase};
use crate::{AccountId, StoreLayout};

#[test]
fn account_registry_persists_one_active_account() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let first = AccountId::new(11).expect("fixture ID should be positive");
    let second = AccountId::new(22).expect("fixture ID should be positive");
    let database = GlobalDatabase::open(&layout).expect("global database should open");

    database
        .register(AccountRecord {
            id: first,
            display_name: "First".to_owned(),
            active: true,
            notification_identity: "telegram:11".to_owned(),
        })
        .expect("first Account should register");
    database
        .register(AccountRecord {
            id: second,
            display_name: "Second".to_owned(),
            active: true,
            notification_identity: "telegram:22".to_owned(),
        })
        .expect("second Account should register");
    drop(database);

    let reopened = GlobalDatabase::open(&layout).expect("global database should reopen");
    let accounts = reopened.accounts().expect("Accounts should load");
    assert_eq!(accounts.len(), 2);
    assert_eq!(accounts[0].id, second);
    assert_eq!(accounts[0].notification_identity, "telegram:22");
    assert!(accounts[0].active);
    assert!(!accounts[1].active);
}

#[test]
fn removing_an_account_leaves_other_registry_records() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let first = AccountId::new(11).expect("fixture ID should be positive");
    let second = AccountId::new(22).expect("fixture ID should be positive");
    let database = GlobalDatabase::open(&layout).expect("global database should open");
    for id in [first, second] {
        database
            .register(AccountRecord {
                id,
                display_name: id.get().to_string(),
                active: id == first,
                notification_identity: format!("telegram:{}", id.get()),
            })
            .expect("account should register");
    }

    database.remove(first).expect("account should be removed");

    assert_eq!(
        database
            .accounts()
            .expect("remaining accounts should load")
            .into_iter()
            .map(|account| account.id)
            .collect::<Vec<_>>(),
        vec![second]
    );
    assert!(
        database
            .accounts()
            .expect("remaining Account should load")
            .first()
            .is_some_and(|account| account.active)
    );
}

#[test]
fn an_existing_global_database_is_backed_up_before_migration() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    fs::create_dir_all(layout.data_directory()).expect("data directory should be created");
    let path = layout.global_database();
    let connection = Connection::open(&path).expect("fixture database should open");
    connection
        .execute("CREATE TABLE legacy(value TEXT NOT NULL)", [])
        .expect("legacy schema should be created");
    drop(connection);

    let database = GlobalDatabase::open(&layout).expect("global database should migrate");
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
