use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread;

use tempfile::tempdir;

use crate::{
    AccountDatabase, AccountId, DatabaseRequest, OutboxAdmission, OutboxExpiry, OutboxOperation,
    OutboxPayload, OutboxPayloadV1, OutboxState, Result, StoreLayout,
};

#[test]
fn fifo_claim_and_explicit_transitions_are_durable() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    let first = database
        .admit_outbox(admission(OutboxOperation::Send, 10, OutboxExpiry::Never))
        .expect("first item should be admitted");
    let second = database
        .admit_outbox(admission(OutboxOperation::Send, 20, OutboxExpiry::Never))
        .expect("second item should be admitted");
    let mutation = database
        .admit_outbox(admission(
            OutboxOperation::Mutation,
            30,
            OutboxExpiry::Never,
        ))
        .expect("mutation should be admitted");

    let claimed = database
        .claim_outbox(100)
        .expect("claim should complete")
        .expect("first item should be eligible");
    assert_eq!(claimed.id, first);
    assert_eq!(claimed.attempts, 1);
    database
        .defer_outbox(first, 200, "flood wait".to_owned())
        .expect("claimed item should defer");
    assert_eq!(
        database
            .claim_outbox(100)
            .expect("second claim should complete")
            .expect("second item should be eligible")
            .id,
        second
    );
    database
        .fail_outbox(second, "permanent".to_owned())
        .expect("claimed item should fail");
    assert_eq!(
        database
            .claim_outbox(100)
            .expect("mutation claim should complete")
            .expect("mutation should be eligible")
            .id,
        mutation
    );
    database
        .conflict_outbox(mutation, "ambiguous".to_owned())
        .expect("claimed mutation should conflict");
    assert!(
        database
            .claim_outbox(199)
            .expect("early retry check should complete")
            .is_none()
    );
    assert_eq!(
        database
            .claim_outbox(200)
            .expect("retry claim should complete")
            .expect("deferred item should become eligible")
            .id,
        first
    );
    database
        .acknowledge_outbox(first, None)
        .expect("acknowledged item should be removed");

    let remaining = database.load_outbox().expect("Outbox should load");
    assert_eq!(remaining.len(), 2);
    assert_eq!(remaining[0].state, OutboxState::Failed);
    assert_eq!(remaining[1].state, OutboxState::Conflict);
}

#[test]
fn ordinary_sends_never_expire_silently() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    assert!(
        database
            .admit_outbox(admission(OutboxOperation::Send, 10, OutboxExpiry::At(20)))
            .is_err()
    );
    let send = database
        .admit_outbox(admission(OutboxOperation::Send, 20, OutboxExpiry::Never))
        .expect("ordinary send should be admitted without expiry");
    let bounded = database
        .admit_outbox(admission(OutboxOperation::Create, 30, OutboxExpiry::At(40)))
        .expect("explicitly bounded create should be admitted");

    assert_eq!(
        database
            .expire_outbox(100)
            .expect("expiry sweep should complete"),
        vec![bounded]
    );
    let records = database.load_outbox().expect("Outbox should load");
    assert_eq!(records[0].id, send);
    assert_eq!(records[0].state, OutboxState::Ready);
    assert_eq!(records[1].state, OutboxState::Expired);
    database
        .cancel_outbox(send)
        .expect("ordinary send should remain cancellable");
    assert_eq!(
        database.load_outbox().expect("Outbox should reload")[0].state,
        OutboxState::Cancelled
    );
}

#[test]
fn reopening_recovers_only_replay_safe_in_flight_work() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let account = AccountId::new(7).expect("fixture Account should be valid");
    let pending =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    let send = pending
        .admit_outbox(admission(OutboxOperation::Send, 10, OutboxExpiry::Never))
        .expect("send should be admitted");
    let create = pending
        .admit_outbox(admission(OutboxOperation::Create, 15, OutboxExpiry::Never))
        .expect("create should be admitted");
    let mutation = pending
        .admit_outbox(admission(
            OutboxOperation::Mutation,
            20,
            OutboxExpiry::Never,
        ))
        .expect("mutation should be admitted");
    assert_eq!(
        pending
            .claim_outbox(100)
            .expect("send claim should complete")
            .expect("send should be eligible")
            .id,
        send
    );
    assert_eq!(
        pending
            .claim_outbox(100)
            .expect("create claim should complete")
            .expect("create should be eligible")
            .id,
        create
    );
    assert_eq!(
        pending
            .claim_outbox(100)
            .expect("mutation claim should complete")
            .expect("mutation should be eligible")
            .id,
        mutation
    );
    let database = pending
        .finish_login(&layout, account)
        .expect("database should promote");
    drop(database);

    let reopened = AccountDatabase::open(&layout, account).expect("database should reopen");
    let records = reopened
        .load_outbox()
        .expect("recovered Outbox should load");
    assert_eq!(records[0].state, OutboxState::Ready);
    assert_eq!(records[1].state, OutboxState::Ready);
    assert_eq!(records[2].state, OutboxState::Conflict);
    assert_eq!(
        records[2].last_error.as_deref(),
        Some("interrupted before mutation acknowledgement")
    );
}

#[test]
fn runtime_endpoint_returns_outbox_results_without_blocking_the_caller() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    let store = database.store();
    let id = wait_for(
        store
            .admit_outbox(admission(OutboxOperation::Send, 10, OutboxExpiry::Never))
            .expect("admission should enqueue"),
    )
    .expect("admission should complete");
    let claimed = wait_for(store.claim_outbox(20).expect("claim should enqueue"))
        .expect("claim should complete")
        .expect("admitted item should be eligible");

    assert_eq!(claimed.id, id);
    assert_eq!(claimed.state, OutboxState::InFlight);
}

fn wait_for<T>(request: DatabaseRequest<T>) -> Result<T> {
    let wake = Waker::from(Arc::new(ThreadWake(thread::current())));
    let mut context = Context::from_waker(&wake);
    let mut request = std::pin::pin!(request);
    loop {
        match request.as_mut().poll(&mut context) {
            Poll::Ready(result) => return result,
            Poll::Pending => thread::park(),
        }
    }
}

struct ThreadWake(thread::Thread);

impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn admission(
    operation: OutboxOperation,
    admitted_at: i64,
    expiry: OutboxExpiry,
) -> OutboxAdmission {
    OutboxAdmission {
        operation,
        payload: OutboxPayload::V1(OutboxPayloadV1 {
            chat_id: 7,
            thread_root: None,
            saved_peer: None,
            local_message_id: None,
            random_id: admitted_at,
            content: admitted_at.to_le_bytes().to_vec(),
        }),
        media: Vec::new(),
        optimistic_message: None,
        consume_draft: false,
        admitted_at,
        expiry,
    }
}
