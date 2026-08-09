use compio_actor::Mailbox;
use intuigram_app::{AdapterEvent, OutboxKey};
use intuigram_store::{AccountStore, OutboxPoll, OutboxRecord};
use snafu::ResultExt;

use super::super::super::{AccountDatabaseSnafu, BackendOutput, Error, Result, outbox_view};
use super::super::actor::TelegramActor;
use super::super::errors::call_error;
use super::policy::{Transition, decide};
use super::{ExecuteOutbox, OutboxResponse};

pub(super) struct Claim {
    pub(super) expired: Vec<OutboxRecord>,
    pub(super) head: Head,
}

pub(super) enum Head {
    Claimed(OutboxRecord),
    Busy,
    WaitingUntil(i64),
    Idle,
}

pub(super) struct Outcome {
    pub(super) outputs: Vec<BackendOutput>,
    pub(super) reconnect: Option<intuigram_telegram::Error>,
}

pub(super) async fn claim(store: AccountStore, now: i64) -> Result<Claim> {
    let expired = store
        .expire_outbox(now)
        .context(AccountDatabaseSnafu)?
        .await
        .context(AccountDatabaseSnafu)?;
    let expired = if expired.is_empty() {
        Vec::new()
    } else {
        store
            .load_outbox()
            .context(AccountDatabaseSnafu)?
            .await
            .context(AccountDatabaseSnafu)?
            .into_iter()
            .filter(|record| expired.contains(&record.id))
            .collect()
    };
    let poll = store
        .claim_outbox(now)
        .context(AccountDatabaseSnafu)?
        .await
        .context(AccountDatabaseSnafu)?;
    let head = match poll {
        OutboxPoll::Claimed(record) => Head::Claimed(record),
        OutboxPoll::Busy { .. } => Head::Busy,
        OutboxPoll::WaitingUntil { available_at, .. } => Head::WaitingUntil(available_at),
        OutboxPoll::Idle => Head::Idle,
    };
    Ok(Claim { expired, head })
}

pub(super) async fn execute(
    store: AccountStore,
    mailbox: Mailbox<TelegramActor>,
    record: OutboxRecord,
    now: i64,
) -> Result<Outcome> {
    let response = mailbox
        .call(ExecuteOutbox(record.clone()))
        .await
        .map_err(call_error)?;
    match response {
        OutboxResponse::Complete(success) => complete(&store, record, *success).await,
        OutboxResponse::ExecutionFailed(error) => failed(&store, record, *error, now).await,
        OutboxResponse::Failed(error) => Err(*error),
        OutboxResponse::Cancelled => Err(Error::TelegramActorCancelled),
    }
}

async fn complete(
    store: &AccountStore,
    record: OutboxRecord,
    success: super::super::super::outbox::execution::Success,
) -> Result<Outcome> {
    store
        .complete_outbox(record.id, success.completion)
        .context(AccountDatabaseSnafu)?
        .await
        .context(AccountDatabaseSnafu)?;
    let mut outputs = vec![BackendOutput::event(Some(AdapterEvent::OutboxRemoved {
        item: OutboxKey(record.id.get()),
    }))];
    if let Some(event) = success.event {
        outputs.push(BackendOutput::event(Some(event)));
    }
    Ok(Outcome {
        outputs,
        reconnect: None,
    })
}

async fn failed(
    store: &AccountStore,
    claimed: OutboxRecord,
    error: super::super::super::outbox::execution::Error,
    now: i64,
) -> Result<Outcome> {
    let reason = error.to_string();
    let disposition = error.retry_disposition();
    let reached_telegram = error.reached_telegram();
    let reconnect = error.into_connection_error();
    let Some(current) = load(store, claimed.id).await? else {
        return Ok(Outcome {
            outputs: vec![BackendOutput::event(Some(AdapterEvent::OutboxRemoved {
                item: OutboxKey(claimed.id.get()),
            }))],
            reconnect,
        });
    };
    let decision = decide(
        current.operation,
        current.state,
        disposition,
        reached_telegram,
        now,
    );
    let request = match decision.transition {
        Transition::Defer(available_at) => store.defer_outbox(current.id, available_at, reason),
        Transition::Fail => store.fail_outbox(current.id, reason),
        Transition::Conflict => store.conflict_outbox(current.id, reason),
        Transition::OutcomeUnknown => store.mark_outbox_outcome_unknown(current.id, reason),
        Transition::Cancel => store.confirm_outbox_unsent(current.id),
    }
    .context(AccountDatabaseSnafu)?;
    request.await.context(AccountDatabaseSnafu)?;
    let outputs = match load(store, current.id).await? {
        Some(record) => vec![BackendOutput::event(Some(AdapterEvent::OutboxChanged(
            outbox_view(record),
        )))],
        None => vec![BackendOutput::event(Some(AdapterEvent::OutboxRemoved {
            item: OutboxKey(current.id.get()),
        }))],
    };
    Ok(Outcome {
        outputs,
        reconnect: decision.reconnect.then_some(reconnect).flatten(),
    })
}

async fn load(store: &AccountStore, id: intuigram_store::OutboxId) -> Result<Option<OutboxRecord>> {
    Ok(store
        .load_outbox()
        .context(AccountDatabaseSnafu)?
        .await
        .context(AccountDatabaseSnafu)?
        .into_iter()
        .find(|record| record.id == id))
}
