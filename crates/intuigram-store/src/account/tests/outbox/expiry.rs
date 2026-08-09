use tempfile::tempdir;

use crate::{
    AccountDatabase, OutboxAdmission, OutboxExpiry, OutboxOperation, OutboxPayload,
    OutboxPayloadV1, OutboxPoll, OutboxState, StoreLayout,
};

#[test]
fn sends_have_no_implicit_expiry_but_accept_caller_chosen_deadlines() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    let unbounded = database
        .admit_outbox(admission(10, OutboxExpiry::Never))
        .expect("unbounded send should be admitted");
    let bounded = database
        .admit_outbox(admission(20, OutboxExpiry::At(50)))
        .expect("explicitly bounded send should be admitted");

    let records = database.load_outbox().expect("Outbox should load");
    assert_eq!(records[0].expires_at, None);
    assert_eq!(records[1].expires_at, Some(50));

    database
        .set_outbox_expiry(unbounded, OutboxExpiry::At(40))
        .expect("ready item should accept a deadline");
    assert_eq!(record(&database, unbounded).expires_at, Some(40));
    database
        .set_outbox_expiry(unbounded, OutboxExpiry::Never)
        .expect("ready item should allow its deadline to be cleared");
    assert_eq!(record(&database, unbounded).expires_at, None);

    assert!(matches!(
        database.claim_outbox(30).expect("claim should complete"),
        OutboxPoll::Claimed(claimed) if claimed.id == unbounded
    ));
    assert!(
        database
            .set_outbox_expiry(unbounded, OutboxExpiry::At(60))
            .is_err()
    );
    assert_eq!(record(&database, bounded).expires_at, Some(50));
}

#[test]
fn polling_expires_a_due_head_and_progresses_to_newer_work() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    let expired = database
        .admit_outbox(admission(10, OutboxExpiry::At(20)))
        .expect("bounded send should be admitted");
    let next = database
        .admit_outbox(admission(20, OutboxExpiry::Never))
        .expect("newer send should be admitted");

    assert!(matches!(
        database.claim_outbox(20).expect("poll should complete"),
        OutboxPoll::Claimed(claimed) if claimed.id == next
    ));
    assert_eq!(record(&database, expired).state, OutboxState::Expired);
}

fn admission(admitted_at: i64, expiry: OutboxExpiry) -> OutboxAdmission {
    OutboxAdmission {
        operation: OutboxOperation::Send,
        payload: payload(admitted_at),
        media: Vec::new(),
        optimistic_message: None,
        consume_draft: false,
        admitted_at,
        expiry,
    }
}

fn payload(random_id: i64) -> OutboxPayload {
    OutboxPayload::V1(OutboxPayloadV1 {
        chat_id: 7,
        thread_root: None,
        saved_peer: None,
        local_message_id: None,
        random_id,
        content: random_id.to_le_bytes().to_vec(),
    })
}

fn record(database: &AccountDatabase, id: crate::OutboxId) -> crate::OutboxRecord {
    database
        .load_outbox()
        .expect("Outbox should load")
        .into_iter()
        .find(|record| record.id == id)
        .expect("Outbox item should remain")
}
