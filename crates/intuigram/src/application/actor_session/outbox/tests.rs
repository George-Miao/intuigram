use intuigram_store::{
    AccountDatabase, AccountId, OutboxAdmission, OutboxExpiry, OutboxOperation, OutboxPayload,
    OutboxPayloadV1, StoreLayout,
};
use tempfile::tempdir;

use super::step::Head;
use super::{Duration, step, wait_duration};

#[test]
fn deferred_head_wait_uses_the_persisted_deadline() {
    assert_eq!(wait_duration(40, 43), Duration::from_secs(3));
    assert_eq!(wait_duration(43, 43), Duration::ZERO);
    assert_eq!(wait_duration(44, 43), Duration::ZERO);
}

#[test]
fn runtime_claims_the_same_fifo_head_after_restart() {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let account = AccountId::new(7).expect("fixture Account should be valid");
    let pending = AccountDatabase::begin_login(&layout).expect("pending Account should open");
    let first = pending
        .admit_outbox(admission(10))
        .expect("first operation should be admitted");
    pending
        .admit_outbox(admission(20))
        .expect("second operation should be admitted");
    let database = pending
        .finish_login(&layout, account)
        .expect("Account database should promote");
    drop(database);

    let database = AccountDatabase::open(&layout, account).expect("Account should reopen");
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");
    let claim = runtime
        .block_on(step::claim(database.store(), 30))
        .expect("runtime claim should complete");
    let Head::Claimed(record) = claim.head else {
        panic!("recovered FIFO head should be claimed")
    };

    assert_eq!(record.id, first);
    assert_eq!(record.attempts, 1);
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
