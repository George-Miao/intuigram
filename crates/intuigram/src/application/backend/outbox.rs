use super::*;

impl Backend {
    pub(super) async fn resolve_outbox(
        &mut self,
        item: OutboxKey,
        action: OutboxAction,
    ) -> Result<Option<AdapterEvent>> {
        let records = self
            .store
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
            OutboxAction::Cancel => self.store.cancel_outbox(id),
            OutboxAction::Retry => self.store.retry_outbox(id),
            OutboxAction::ResolveConflict => self.store.resolve_outbox_conflict(id, record.payload),
            OutboxAction::ResolveOutcomeUnknown => self.store.resolve_outbox_outcome_unknown(id),
            OutboxAction::Dismiss => self.store.dismiss_outbox(id),
        }
        .context(AccountDatabaseSnafu)?;
        request.await.context(AccountDatabaseSnafu)?;
        if action == OutboxAction::Dismiss {
            return Ok(Some(AdapterEvent::OutboxRemoved { item }));
        }
        let record = self
            .store
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
}
