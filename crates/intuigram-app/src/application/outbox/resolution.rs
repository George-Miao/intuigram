use intuigram_lib::{AdapterEvent, OutboxAction, OutboxKey};
use intuigram_store::AccountStore;
use snafu::ResultExt;

use super::super::{AccountDatabaseSnafu, Result, outbox_view};

pub(in crate::application) async fn execute(
    store: &AccountStore,
    item: OutboxKey,
    action: OutboxAction,
) -> Result<Option<AdapterEvent>> {
    let records = store
        .load_outbox()
        .context(AccountDatabaseSnafu)?
        .await
        .context(AccountDatabaseSnafu)?;
    let Some(record) = records.into_iter().find(|record| record.id.get() == item.0) else {
        return Ok(Some(AdapterEvent::OperationFailed(format!(
            "Outbox item {} no longer exists",
            item.0
        ))));
    };
    let id = record.id;
    let request = match action {
        OutboxAction::Cancel => store.cancel_outbox(id),
        OutboxAction::Retry => store.retry_outbox(id),
        OutboxAction::ResolveConflict => store.resolve_outbox_conflict(id, record.payload),
        OutboxAction::ResolveOutcomeUnknown => store.resolve_outbox_outcome_unknown(id),
        OutboxAction::Dismiss => store.dismiss_outbox(id),
    }
    .context(AccountDatabaseSnafu)?;
    if let Err(source) = request.await {
        let still_exists = store
            .load_outbox()
            .context(AccountDatabaseSnafu)?
            .await
            .context(AccountDatabaseSnafu)?
            .into_iter()
            .any(|record| record.id == id);
        if !still_exists {
            return Ok(Some(AdapterEvent::OutboxRemoved { item }));
        }
        return Err(super::super::Error::AccountDatabase { source });
    }
    if action == OutboxAction::Dismiss {
        return Ok(Some(AdapterEvent::OutboxRemoved { item }));
    }
    let record = store
        .load_outbox()
        .context(AccountDatabaseSnafu)?
        .await
        .context(AccountDatabaseSnafu)?
        .into_iter()
        .find(|record| record.id == id);
    Ok(Some(match record {
        Some(record) => AdapterEvent::OutboxChanged(outbox_view(record)),
        None => AdapterEvent::OutboxRemoved { item },
    }))
}
