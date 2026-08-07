use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread;

use intuigram_app::{
    AdapterEvent, App, ChatId, DeliveryState, Input, MediaCard, MediaKind, MessageDetails,
    MessageDirection, MessageId, MessageView, PollOptionView, PollView,
};
use intuigram_store::{AccountDatabase, StoreLayout, SyncCursor};
use intuigram_telegram::{LiveEvent, UpdateCursor, UpdateScope};
use tempfile::tempdir;

use super::{UpdateCommitter, decode_stored_message, encode_stored_message};

#[test]
fn rich_media_and_album_state_round_trip_through_the_cache() {
    let message = MessageView {
        id: MessageId(42),
        sender: "Ada".to_owned(),
        body: "Choose".to_owned(),
        timestamp: "12:00".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: Some(MessageId(40)),
        details: MessageDetails {
            date_label: "2026-08-07".to_owned(),
            media: Some(MediaCard {
                kind: MediaKind::Poll,
                title: "Quiz".to_owned(),
                description: "Which runtime?".to_owned(),
                details: vec!["current results".to_owned()],
                poll: Some(PollView {
                    quiz: true,
                    multiple_choice: false,
                    closed: true,
                    total_voters: Some(5),
                    options: vec![PollOptionView {
                        text: "Compio".to_owned(),
                        voters: Some(3),
                        chosen: true,
                        correct: true,
                    }],
                    solution: Some("Completion-based I/O".to_owned()),
                }),
                remote_id: Some("77".to_owned()),
            }),
            album_id: Some(9),
            ..MessageDetails::default()
        },
    };

    let stored = encode_stored_message(ChatId(7), &message);

    assert_eq!(decode_stored_message(stored), message);
}

#[test]
fn message_update_for_an_uncached_chat_is_committed_with_its_cursor() {
    let temporary = tempdir().expect("temporary data directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("Account database should be created");
    let store = database.store();
    let mut committer = UpdateCommitter::new(store, [SyncCursor::default()], []);
    let mut peers = intuigram_telegram::PeerDirectory::default();
    peers.insert(intuigram_telegram::PeerAddress::User {
        id: 77,
        access_hash: 9,
    });
    let update = LiveEvent {
        events: vec![AdapterEvent::MessageAdded {
            chat: ChatId(77),
            message: Box::new(MessageView {
                id: MessageId(42),
                sender: "Ada".to_owned(),
                body: "hello".to_owned(),
                timestamp: "12:00".to_owned(),
                direction: MessageDirection::Incoming,
                delivery: DeliveryState::Sent,
                reply_to: None,
                details: MessageDetails::default(),
            }),
        }],
        cursors: vec![UpdateCursor {
            pts: Some(9),
            date: Some(10),
            seq: Some(2),
            seq_start: Some(2),
            ..UpdateCursor::default()
        }],
        peers,
    };

    let commit = committer
        .commit(update)
        .expect("update should be accepted by the database worker");
    let committed =
        block_on(commit).expect("an update for a previously uncached Chat should commit");
    assert!(committed.peers.contains(ChatId(77)));

    let cached = database
        .cached_account()
        .expect("durable cache should remain readable");
    assert_eq!(cached.cursors[0].pts, 9);
    assert_eq!(cached.chats[0].id, 77);
    assert_eq!(cached.messages[0].chat_id, 77);
}

#[test]
fn uncached_chat_discovery_carries_live_pin_permission_into_the_app() {
    let temporary = tempdir().expect("temporary data directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("Account database should be created");
    let mut committer = UpdateCommitter::new(database.store(), Vec::new(), []);
    let chat = ChatId(-1_000_000_000_077);
    let update = LiveEvent {
        events: vec![
            AdapterEvent::ChatPinPermissionChanged {
                chat,
                can_pin_messages: true,
            },
            AdapterEvent::MessageAdded {
                chat,
                message: Box::new(MessageView {
                    id: MessageId(42),
                    sender: "Ada".to_owned(),
                    body: "hello".to_owned(),
                    timestamp: "12:00".to_owned(),
                    direction: MessageDirection::Incoming,
                    delivery: DeliveryState::Sent,
                    reply_to: None,
                    details: MessageDetails::default(),
                }),
            },
        ],
        cursors: Vec::new(),
        peers: intuigram_telegram::PeerDirectory::default(),
    };

    let committed = block_on(
        committer
            .commit(update)
            .expect("unknown Chat update should be accepted"),
    )
    .expect("unknown Chat update should commit");
    assert!(committed.events.iter().any(|event| matches!(
        event,
        AdapterEvent::ChatDiscovered { chat: discovered }
            if discovered.id == chat && discovered.can_pin_messages
    )));

    let mut app = App::new();
    for event in committed.events {
        drop(app.transition(Input::Adapter(event)));
    }
    assert!(app.view().chats[0].can_pin_messages);
}

#[test]
fn archive_update_for_an_uncached_chat_creates_its_parent_record() {
    let temporary = tempdir().expect("temporary data directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("Account database should be created");
    let mut committer = UpdateCommitter::new(database.store(), [SyncCursor::default()], []);
    let update = LiveEvent {
        events: vec![AdapterEvent::ChatArchiveChanged {
            chat: ChatId(88),
            archived: true,
        }],
        cursors: vec![UpdateCursor {
            pts: Some(11),
            ..UpdateCursor::default()
        }],
        peers: intuigram_telegram::PeerDirectory::default(),
    };

    block_on(
        committer
            .commit(update)
            .expect("update should be accepted by the database worker"),
    )
    .expect("an archive update for a previously uncached Chat should commit");

    let cached = database
        .cached_account()
        .expect("durable cache should remain readable");
    assert_eq!(cached.chats[0].id, 88);
    assert_eq!(cached.chats[0].folders, vec![-1]);
}

#[test]
fn a_pts_gap_is_rejected_before_records_or_cursors_are_committed() {
    let temporary = tempdir().expect("temporary data directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("Account database should be created");
    let baseline = SyncCursor {
        scope: "account".to_owned(),
        pts: 10,
        ..SyncCursor::default()
    };
    let mut committer = UpdateCommitter::new(database.store(), [baseline], []);

    let result = committer.commit(LiveEvent {
        events: Vec::new(),
        cursors: vec![UpdateCursor {
            pts: Some(12),
            pts_count: 1,
            ..UpdateCursor::default()
        }],
        peers: intuigram_telegram::PeerDirectory::default(),
    });
    let Err(error) = result else {
        panic!("missing pts 11 must require reconciliation")
    };

    assert!(error.requires_reconnect());
    assert!(
        database
            .cached_account()
            .expect("cache should remain readable")
            .cursors
            .is_empty()
    );
}

#[test]
fn a_global_sequence_gap_requires_reconciliation() {
    let temporary = tempdir().expect("temporary data directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("Account database should be created");
    let baseline = SyncCursor {
        scope: "account".to_owned(),
        seq: 5,
        ..SyncCursor::default()
    };
    let mut committer = UpdateCommitter::new(database.store(), [baseline], []);

    let result = committer.commit(LiveEvent {
        events: Vec::new(),
        cursors: vec![UpdateCursor {
            seq: Some(7),
            seq_start: Some(7),
            ..UpdateCursor::default()
        }],
        peers: intuigram_telegram::PeerDirectory::default(),
    });

    assert!(matches!(result, Err(error) if error.requires_reconnect()));
}

#[test]
fn account_and_channel_cursors_commit_in_one_transaction() {
    let temporary = tempdir().expect("temporary data directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("Account database should be created");
    let mut committer = UpdateCommitter::new(database.store(), Vec::new(), []);

    block_on(
        committer
            .commit(LiveEvent {
                events: Vec::new(),
                cursors: vec![
                    UpdateCursor {
                        pts: Some(7),
                        ..UpdateCursor::default()
                    },
                    UpdateCursor {
                        scope: UpdateScope::Channel(ChatId(-1_000_000_000_005)),
                        pts: Some(30),
                        ..UpdateCursor::default()
                    },
                ],
                peers: intuigram_telegram::PeerDirectory::default(),
            })
            .expect("scoped cursor update should be accepted"),
    )
    .expect("scoped cursors should commit");

    let cached = database
        .cached_account()
        .expect("cache should remain readable");
    assert_eq!(cached.cursors.len(), 2);
    assert_eq!(cached.cursors[0].scope, "account");
    assert_eq!(cached.cursors[1].scope, "channel:-1000000000005");
}

fn block_on<F: Future>(future: F) -> F::Output {
    struct ThreadWake(thread::Thread);

    impl Wake for ThreadWake {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }

    let mut future = pin!(future);
    let waker = Waker::from(Arc::new(ThreadWake(thread::current())));
    let mut context = Context::from_waker(&waker);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => thread::park(),
        }
    }
}
