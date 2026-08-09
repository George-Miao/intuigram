use tempfile::tempdir;

use super::pinned::block_on;
use super::{AccountDatabase, StoredMessage};
use crate::StoreLayout;
use crate::account::StoredMutation;

#[test]
fn paid_media_child_update_preserves_price_and_surrounding_metadata() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    let mut bootstrap = super::sync_batch();
    bootstrap.messages.clear();
    database
        .commit_sync(bootstrap)
        .expect("parent Chat should persist");
    let metadata = r#"{"pinned":true,"media":{"title":"Paid media","description":"merchant disclosure","specialized":{"kind":"paid_media","stars_amount":50,"items":[{"state":"preview","width":640,"height":480,"duration_seconds":null}]}}}"#;
    let save = database
        .store()
        .save_messages(vec![StoredMessage {
            chat_id: 7,
            id: 81,
            sender: "Ada".to_owned(),
            body: "[Paid media]".to_owned(),
            timestamp: "12:00".to_owned(),
            direction: "incoming".to_owned(),
            delivery: "read".to_owned(),
            reply_to: None,
            thread_root: None,
            saved_peer: None,
            content_kind: "paidmedia".to_owned(),
            metadata: metadata.to_owned(),
        }])
        .expect("paid Message should enqueue");
    block_on(save).expect("paid Message should persist");
    let mut update = super::sync_batch();
    update.cursors.clear();
    update.folders.clear();
    update.chats.clear();
    update.messages.clear();
    update.mutations = vec![StoredMutation::SetPaidMediaItems {
        chat_id: 7,
        message_id: 81,
        items: r#"[{"state":"available","media_kind":"photo","title":"Photo","remote_id":"photo-900"}]"#.to_owned(),
    }];
    database
        .commit_sync(update)
        .expect("paid child update should commit atomically");

    let cached = database.cached_account().expect("cache should load");
    let stored = cached
        .messages
        .iter()
        .find(|message| message.id == 81)
        .expect("paid Message should remain");
    assert!(stored.metadata.contains(r#""stars_amount":50"#));
    assert!(stored.metadata.contains(r#""state":"available""#));
    assert!(stored.metadata.contains(r#""remote_id":"photo-900""#));
    assert!(
        stored
            .metadata
            .contains(r#""description":"merchant disclosure""#)
    );
    assert!(stored.metadata.contains(r#""pinned":true"#));
    assert!(!stored.metadata.contains(r#""state":"preview""#));
}
