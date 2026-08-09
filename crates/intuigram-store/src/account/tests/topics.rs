use tempfile::tempdir;

use super::{AccountDatabase, sync_batch};
use crate::{StoreLayout, StoredTopic};

#[test]
fn topic_order_and_state_survive_restart() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    let mut batch = sync_batch();
    batch.chats[0].has_topics = true;
    database
        .commit_sync(batch)
        .expect("forum Chat should persist");
    let topics = vec![
        topic(7, 40, "Pinned", 3, true),
        topic(7, 1, "General", 0, false),
    ];
    database
        .save_topics(7, topics.clone())
        .expect("Topic projection should persist");

    assert_eq!(
        database
            .cached_account()
            .expect("Topic cache should load")
            .topics,
        topics
    );
    drop(database);

    let database = AccountDatabase::begin_login(&layout)
        .expect("pending database should reopen after restart");
    let cached = database
        .cached_account()
        .expect("restarted Topic cache should load");
    assert!(cached.chats[0].has_topics);
    assert_eq!(cached.topics, topics);
}

fn topic(chat_id: i64, id: i64, title: &str, unread: u32, pinned: bool) -> StoredTopic {
    StoredTopic {
        chat_id,
        id,
        title: title.to_owned(),
        preview: format!("latest in {title}"),
        timestamp: "12:00".to_owned(),
        unread,
        pinned,
        closed: false,
        hidden: id == 1,
        icon_color: 0x6f_76_5b,
        icon_emoji_id: None,
        top_message_id: Some(id + 1),
        draft_text: (id == 40).then(|| "Topic Draft".to_owned()),
        draft_reply_to: None,
    }
}
