use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread;

use tempfile::{TempDir, tempdir};

use super::{AccountDatabase, StoredMessage};
use crate::StoreLayout;
use crate::account::StoredMutation;

#[test]
fn pinned_projection_is_cached_separately_from_recent_history() {
    let (_temporary, database, recent, old_pin) = history_with_old_pin();

    let cached = database.cached_account().expect("cache should load");
    assert_eq!(cached.messages, vec![recent]);
    assert_eq!(cached.pinned_messages, vec![old_pin]);
}

#[test]
fn unpinned_projection_does_not_join_recent_history_after_restart() {
    let (temporary, database, recent, _) = history_with_old_pin();
    let mut update = super::sync_batch();
    update.cursors.clear();
    update.folders.clear();
    update.chats.clear();
    update.messages.clear();
    update.mutations = vec![StoredMutation::SetMessagesPinned {
        chat_id: 7,
        ids: vec![5],
        pinned: false,
    }];

    database
        .commit_sync(update)
        .expect("unpin mutation should persist");

    let reopened = reopen(&temporary, database);
    let cached = reopened.cached_account().expect("cache should reload");
    assert_eq!(cached.messages, vec![recent]);
    assert!(cached.pinned_messages.is_empty());
}

#[test]
fn refreshed_projection_does_not_join_recent_history_after_restart() {
    let (temporary, database, recent, _) = history_with_old_pin();
    let request = database
        .store()
        .save_chat_history(7, vec![recent.clone()], Vec::new(), None)
        .expect("history refresh should enqueue");

    block_on(request).expect("history refresh should persist");

    let reopened = reopen(&temporary, database);
    let cached = reopened.cached_account().expect("cache should reload");
    assert_eq!(cached.messages, vec![recent]);
    assert!(cached.pinned_messages.is_empty());
}

#[test]
fn history_refresh_persists_richer_chat_status_with_the_messages() {
    let (temporary, database, recent, old_pin) = history_with_old_pin();
    let refresh = database
        .store()
        .save_chat_history(
            7,
            vec![recent],
            vec![old_pin],
            Some("240 members, 31 online".to_owned()),
        )
        .expect("metadata refresh should enqueue");
    block_on(refresh).expect("metadata refresh should persist");

    let reopened = reopen(&temporary, database);
    let cached = reopened.cached_account().expect("cache should reload");
    let chat = cached
        .chats
        .iter()
        .find(|chat| chat.id == 7)
        .expect("parent Chat should remain");
    assert_eq!(chat.status, "240 members, 31 online");
}

#[test]
fn refreshed_history_prunes_stale_acknowledged_rows_only_from_its_recent_window() {
    let (temporary, database, recent, old_pin) = history_with_old_pin();
    let older = stored_message(50, "older history");
    let stale = stored_message(101, "deleted on Telegram");
    let mut pending = stored_message(-1, "pending send");
    pending.delivery = "pending".to_owned();
    let save = database
        .store()
        .save_messages(vec![older.clone(), stale.clone(), pending.clone()])
        .expect("cached Messages should enqueue");
    block_on(save).expect("cached Messages should persist");
    let newest = stored_message(102, "newest");
    let refresh = database
        .store()
        .save_chat_history(7, vec![recent.clone(), newest.clone()], vec![old_pin], None)
        .expect("history refresh should enqueue");
    block_on(refresh).expect("history refresh should persist");

    let reopened = reopen(&temporary, database);
    let cached = reopened.cached_account().expect("cache should reload");
    assert!(cached.messages.contains(&older));
    assert!(cached.messages.contains(&recent));
    assert!(cached.messages.contains(&newest));
    assert!(cached.messages.contains(&pending));
    assert!(!cached.messages.contains(&stale));
}

#[test]
fn server_acknowledgement_replaces_the_optimistic_row_across_restart() {
    let (temporary, database, ..) = history_with_old_pin();
    let mut local = stored_message(-1, "optimistic media");
    local.direction = "outgoing".to_owned();
    local.delivery = "pending".to_owned();
    let save = database
        .store()
        .save_messages(vec![local.clone()])
        .expect("optimistic Message should enqueue");
    block_on(save).expect("optimistic Message should persist");

    let mut server = local;
    server.id = 77;
    server.delivery = "sent".to_owned();
    let replace = database
        .store()
        .replace_message(7, -1, server.clone())
        .expect("Message identity replacement should enqueue");
    block_on(replace).expect("Message identity replacement should persist atomically");

    let reopened = reopen(&temporary, database);
    let cached = reopened.cached_account().expect("cache should reload");
    assert!(cached.messages.contains(&server));
    assert!(!cached.messages.iter().any(|message| message.id == -1));
}

fn history_with_old_pin() -> (TempDir, AccountDatabase, StoredMessage, StoredMessage) {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    let mut batch = super::sync_batch();
    batch.messages.clear();
    database
        .commit_sync(batch)
        .expect("parent Chat should persist");
    let recent = stored_message(100, "recent");
    let old_pin = stored_message(5, "old pin");
    let request = database
        .store()
        .save_chat_history(7, vec![recent.clone()], vec![old_pin.clone()], None)
        .expect("history save should enqueue");
    block_on(request).expect("history projection should persist");
    (temporary, database, recent, old_pin)
}

fn reopen(temporary: &TempDir, database: AccountDatabase) -> AccountDatabase {
    drop(database);
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    AccountDatabase::begin_login(&layout).expect("account database should reopen")
}

fn stored_message(id: i64, body: &str) -> StoredMessage {
    StoredMessage {
        chat_id: 7,
        id,
        sender: "Ada".to_owned(),
        body: body.to_owned(),
        timestamp: "12:00".to_owned(),
        direction: "incoming".to_owned(),
        delivery: "read".to_owned(),
        reply_to: None,
        thread_root: None,
        saved_peer: None,
        content_kind: "text".to_owned(),
        metadata: "{}".to_owned(),
    }
}

struct ThreadWaker(thread::Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => thread::park(),
        }
    }
}
