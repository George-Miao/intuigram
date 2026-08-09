use tempfile::tempdir;

use super::{AccountDatabase, StoredSelection, sync_batch};
use crate::{StoreLayout, StoredDraft, StoredSavedDialog, StoredTranscriptAnchor};

#[test]
fn saved_dialog_order_and_filtered_anchor_survive_restart() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    database
        .commit_sync(sync_batch())
        .expect("Saved Messages Chat should persist");
    let dialogs = vec![
        dialog(7, 9, "Pinned origin", true),
        dialog(7, 11, "Ada", false),
    ];
    database
        .save_saved_dialogs(7, dialogs.clone())
        .expect("saved dialog projection should persist");
    database
        .save_selection(StoredSelection {
            folder_id: 0,
            chat_id: Some(7),
            anchor_message_id: None,
            transcript_anchors: vec![StoredTranscriptAnchor {
                chat_id: 7,
                thread_root: None,
                saved_peer: Some(9),
                message_id: 42,
            }],
        })
        .expect("Saved Messages anchor should persist");

    drop(database);
    let database = AccountDatabase::begin_login(&layout)
        .expect("pending database should reopen after restart");
    let cached = database
        .cached_account()
        .expect("restarted Saved Messages cache should load");

    assert_eq!(cached.saved_dialogs, dialogs);
    assert_eq!(
        cached
            .selection
            .expect("selection should remain")
            .transcript_anchors[0]
            .saved_peer,
        Some(9)
    );
}

#[test]
fn monoforum_user_drafts_remain_independent() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");

    for (peer, text) in [(9, "reply to Ada"), (11, "reply to Lin")] {
        database
            .save_draft(StoredDraft {
                chat_id: 7,
                thread_root: None,
                saved_peer: Some(peer),
                text: text.to_owned(),
                reply_to: None,
                modified_at: peer,
            })
            .expect("peer-scoped Draft should persist");
    }

    let cached = database
        .cached_account()
        .expect("peer-scoped Drafts should load");
    assert_eq!(cached.drafts.len(), 2);
    assert_eq!(cached.drafts[0].saved_peer, Some(9));
    assert_eq!(cached.drafts[1].saved_peer, Some(11));
}

fn dialog(chat_id: i64, peer_id: i64, title: &str, pinned: bool) -> StoredSavedDialog {
    StoredSavedDialog {
        chat_id,
        peer_id,
        title: title.to_owned(),
        preview: format!("saved from {title}"),
        timestamp: "12:00".to_owned(),
        pinned,
        unread: 0,
        unread_mark: false,
        top_message_id: peer_id + 100,
        draft_text: None,
        draft_reply_to: None,
    }
}
