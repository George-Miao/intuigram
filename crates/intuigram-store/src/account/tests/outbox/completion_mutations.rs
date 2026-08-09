use tempfile::{TempDir, tempdir};

use super::super::sync_batch;
use crate::{
    AccountDatabase, OutboxAdmission, OutboxCompletion, OutboxExpiry, OutboxOperation,
    OutboxPayload, OutboxPayloadV1, OutboxPoll, OutboxState, StoreLayout, StoredMutation,
};

#[test]
fn cancel_requested_mutation_completion_applies_only_its_normalized_result() {
    let (_temporary, database, first, second) = database_with_two_messages();
    let id = admit_mutation(&database);
    claim(&database, id);
    database
        .cancel_outbox(id)
        .expect("cancellation should be requested");

    database
        .complete_outbox(
            id,
            OutboxCompletion::Mutations(vec![StoredMutation::DeleteMessages {
                chat_id: Some(7),
                ids: vec![first],
            }]),
        )
        .expect("late normalized mutation result should win");

    let cached = database.cached_account().expect("cache should load");
    assert!(cached.outbox.is_empty());
    assert!(!cached.messages.iter().any(|message| message.id == first));
    assert!(cached.messages.iter().any(|message| message.id == second));
}

#[test]
fn failed_mutation_batch_rolls_back_results_and_outbox_removal_together() {
    let (_temporary, database, first, _) = database_with_two_messages();
    let id = admit_mutation(&database);
    claim(&database, id);

    assert!(
        database
            .complete_outbox(
                id,
                OutboxCompletion::Mutations(vec![
                    StoredMutation::SetMessagesPinned {
                        chat_id: 7,
                        ids: vec![first],
                        pinned: true,
                    },
                    StoredMutation::SetPaidMediaItems {
                        chat_id: 7,
                        message_id: first,
                        items: "not json".to_owned(),
                    },
                ]),
            )
            .is_err()
    );

    let cached = database.cached_account().expect("cache should load");
    assert_eq!(cached.outbox[0].state, OutboxState::InFlight);
    assert_eq!(
        cached
            .messages
            .iter()
            .find(|message| message.id == first)
            .expect("target Message should remain")
            .metadata,
        "{}"
    );
}

#[test]
fn completion_shape_must_match_the_claimed_operation() {
    let (_temporary, database, first, _) = database_with_two_messages();
    let id = database
        .admit_outbox(admission(OutboxOperation::Send))
        .expect("send should be admitted");
    claim(&database, id);

    assert!(
        database
            .complete_outbox(
                id,
                OutboxCompletion::Mutations(vec![StoredMutation::DeleteMessages {
                    chat_id: Some(7),
                    ids: vec![first],
                }]),
            )
            .is_err()
    );

    let cached = database.cached_account().expect("cache should load");
    assert_eq!(cached.outbox[0].state, OutboxState::InFlight);
    assert!(cached.messages.iter().any(|message| message.id == first));
}

fn database_with_two_messages() -> (TempDir, AccountDatabase, i64, i64) {
    let temporary = tempdir().expect("temporary directory should be created");
    let layout = StoreLayout::new(temporary.path().join("intuigram"));
    let database =
        AccountDatabase::begin_login(&layout).expect("pending login database should open");
    let mut batch = sync_batch();
    batch.messages[0].metadata = "{}".to_owned();
    let first = batch.messages[0].id;
    let mut unrelated = batch.messages[0].clone();
    unrelated.id = 43;
    let second = unrelated.id;
    batch.messages.push(unrelated);
    database
        .commit_sync(batch)
        .expect("Message fixtures should persist");
    (temporary, database, first, second)
}

fn admit_mutation(database: &AccountDatabase) -> crate::OutboxId {
    database
        .admit_outbox(admission(OutboxOperation::Mutation))
        .expect("mutation should be admitted")
}

fn admission(operation: OutboxOperation) -> OutboxAdmission {
    OutboxAdmission {
        operation,
        payload: OutboxPayload::V1(OutboxPayloadV1 {
            chat_id: 7,
            thread_root: None,
            saved_peer: None,
            local_message_id: None,
            random_id: 17,
            content: b"operation".to_vec(),
        }),
        media: Vec::new(),
        optimistic_message: None,
        consume_draft: false,
        admitted_at: 10,
        expiry: OutboxExpiry::Never,
    }
}

fn claim(database: &AccountDatabase, expected: crate::OutboxId) {
    assert!(matches!(
        database.claim_outbox(20).expect("claim should complete"),
        OutboxPoll::Claimed(record) if record.id == expected
    ));
}
