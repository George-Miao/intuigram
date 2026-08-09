use tempfile::tempdir;

use super::super::AccountDatabase;
use crate::StoreLayout;

#[test]
fn offline_media_policy_is_account_local_and_durable() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");

    database
        .set_chat_media_offline(7, true)
        .expect("offline policy should persist");
    database
        .set_chat_media_offline(9, true)
        .expect("second offline policy should persist");
    assert_eq!(
        database
            .cached_account()
            .expect("cached policy should load")
            .offline_chats,
        vec![7, 9]
    );

    drop(database);
    let database = AccountDatabase::begin_login(&layout)
        .expect("pending database should reopen after restart");
    assert_eq!(
        database
            .cached_account()
            .expect("restarted policy should load")
            .offline_chats,
        vec![7, 9]
    );

    database
        .set_chat_media_offline(7, false)
        .expect("offline policy should clear");
    assert_eq!(
        database
            .cached_account()
            .expect("updated policy should load")
            .offline_chats,
        vec![9]
    );
}
