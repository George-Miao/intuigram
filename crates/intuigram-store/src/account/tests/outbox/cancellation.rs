use tempfile::tempdir;

use crate::{
    AccountDatabase, OutboxAdmission, OutboxExpiry, OutboxOperation, OutboxPayload,
    OutboxPayloadV1, OutboxPoll, OutboxState, StoreLayout,
};

#[test]
fn cancellation_distinguishes_unstarted_and_in_flight_work() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    let ready = database
        .admit_outbox(admission(10))
        .expect("ready item should be admitted");

    database
        .cancel_outbox(ready)
        .expect("unstarted item should cancel immediately");
    assert_eq!(state(&database, ready), OutboxState::Cancelled);

    let deferred = database
        .admit_outbox(admission(15))
        .expect("deferred item should be admitted");
    claim(&database, deferred);
    database
        .defer_outbox(deferred, 100, "retry later".to_owned())
        .expect("claimed item should defer");
    database
        .cancel_outbox(deferred)
        .expect("deferred item should cancel immediately");
    assert_eq!(state(&database, deferred), OutboxState::Cancelled);

    let in_flight = database
        .admit_outbox(admission(20))
        .expect("in-flight item should be admitted");
    assert!(matches!(
        database.claim_outbox(30).expect("claim should complete"),
        OutboxPoll::Claimed(record) if record.id == in_flight
    ));
    database
        .cancel_outbox(in_flight)
        .expect("in-flight cancellation should be requested");

    assert_eq!(state(&database, in_flight), OutboxState::CancelRequested);
    assert_eq!(
        database
            .claim_outbox(30)
            .expect("cancel-requested poll should complete"),
        OutboxPoll::Busy { id: in_flight }
    );
}

#[test]
fn definitely_unsent_confirmation_releases_the_fifo_head() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    let first = database
        .admit_outbox(admission(10))
        .expect("first item should be admitted");
    let second = database
        .admit_outbox(admission(20))
        .expect("second item should be admitted");
    claim(&database, first);
    database
        .cancel_outbox(first)
        .expect("cancellation should be requested");

    database
        .confirm_outbox_unsent(first)
        .expect("definitely-unsent item should finish cancellation");

    assert_eq!(state(&database, first), OutboxState::Cancelled);
    assert!(matches!(
        database.claim_outbox(30).expect("next claim should complete"),
        OutboxPoll::Claimed(record) if record.id == second
    ));
}

#[test]
fn unknown_outcome_releases_the_fifo_head_for_explicit_resolution() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    let first = database
        .admit_outbox(admission(10))
        .expect("first item should be admitted");
    let second = database
        .admit_outbox(admission(20))
        .expect("second item should be admitted");
    claim(&database, first);
    database
        .cancel_outbox(first)
        .expect("cancellation should be requested");

    database
        .mark_outbox_outcome_unknown(first, "connection lost".to_owned())
        .expect("unknown result should become explicit");

    let records = database.load_outbox().expect("Outbox should load");
    assert_eq!(records[0].state, OutboxState::OutcomeUnknown);
    assert_eq!(records[0].last_error.as_deref(), Some("connection lost"));
    assert!(matches!(
        database.claim_outbox(30).expect("next claim should complete"),
        OutboxPoll::Claimed(record) if record.id == second
    ));
}

#[test]
fn acknowledgement_wins_after_cancellation_was_requested() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    let id = database
        .admit_outbox(admission(10))
        .expect("item should be admitted");
    claim(&database, id);
    database
        .cancel_outbox(id)
        .expect("cancellation should be requested");

    database
        .acknowledge_outbox(id, None)
        .expect("late acknowledgement should win the race");

    assert!(
        database
            .load_outbox()
            .expect("Outbox should load")
            .is_empty()
    );
}

fn admission(admitted_at: i64) -> OutboxAdmission {
    OutboxAdmission {
        operation: OutboxOperation::Send,
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
        expiry: OutboxExpiry::Never,
    }
}

fn claim(database: &AccountDatabase, expected: crate::OutboxId) {
    assert!(matches!(
        database.claim_outbox(30).expect("claim should complete"),
        OutboxPoll::Claimed(record) if record.id == expected
    ));
}

fn state(database: &AccountDatabase, id: crate::OutboxId) -> OutboxState {
    database
        .load_outbox()
        .expect("Outbox should load")
        .into_iter()
        .find(|record| record.id == id)
        .expect("Outbox item should remain")
        .state
}
