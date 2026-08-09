use super::{OutboxCommand, Reply};
use crate::account::worker::{Command, async_response, map_try_send_error};
use crate::account::{
    AccountStore, DatabaseRequest, OutboxAdmission, OutboxExpiry, OutboxId, OutboxPayload,
    OutboxPoll, OutboxRecord, Result, StoredMessage,
};

impl AccountStore {
    fn outbox_request<T>(
        &self,
        build: impl FnOnce(Reply<T>) -> OutboxCommand,
    ) -> Result<DatabaseRequest<T>> {
        let (reply, request) = async_response();
        self.commands
            .try_send(Command::Outbox(build(Reply::Async(reply))))
            .map_err(map_try_send_error)?;
        Ok(request)
    }

    /// Enqueues an atomic durable Outbox admission.
    pub fn admit_outbox(&self, admission: OutboxAdmission) -> Result<DatabaseRequest<OutboxId>> {
        self.outbox_request(|reply| OutboxCommand::Admit {
            admission: Box::new(admission),
            reply,
        })
    }

    /// Enqueues a FIFO Outbox load.
    pub fn load_outbox(&self) -> Result<DatabaseRequest<Vec<OutboxRecord>>> {
        self.outbox_request(|reply| OutboxCommand::Load { reply })
    }

    /// Enqueues a poll and possible claim of the single FIFO Outbox head.
    pub fn claim_outbox(&self, now: i64) -> Result<DatabaseRequest<OutboxPoll>> {
        self.outbox_request(|reply| OutboxCommand::Claim { now, reply })
    }

    /// Enqueues an explicit retry deferral.
    pub fn defer_outbox(
        &self,
        id: OutboxId,
        available_at: i64,
        reason: String,
    ) -> Result<DatabaseRequest<()>> {
        self.outbox_request(|reply| OutboxCommand::Defer {
            id,
            available_at,
            reason,
            reply,
        })
    }

    /// Enqueues a permanent failure transition.
    pub fn fail_outbox(&self, id: OutboxId, reason: String) -> Result<DatabaseRequest<()>> {
        self.outbox_request(|reply| OutboxCommand::Fail { id, reason, reply })
    }

    /// Enqueues a replay conflict transition.
    pub fn conflict_outbox(&self, id: OutboxId, reason: String) -> Result<DatabaseRequest<()>> {
        self.outbox_request(|reply| OutboxCommand::Conflict { id, reason, reply })
    }

    /// Enqueues expiration of explicitly bounded work.
    pub fn expire_outbox(&self, now: i64) -> Result<DatabaseRequest<Vec<OutboxId>>> {
        self.outbox_request(|reply| OutboxCommand::Expire { now, reply })
    }

    /// Enqueues setting or clearing a caller-chosen non-active deadline.
    pub fn set_outbox_expiry(
        &self,
        id: OutboxId,
        expiry: OutboxExpiry,
    ) -> Result<DatabaseRequest<()>> {
        self.outbox_request(|reply| OutboxCommand::SetExpiry { id, expiry, reply })
    }

    /// Enqueues retry of replay-safe failed work.
    pub fn retry_outbox(&self, id: OutboxId) -> Result<DatabaseRequest<()>> {
        self.outbox_request(|reply| OutboxCommand::Retry { id, reply })
    }

    /// Enqueues conflict resolution with a replacement versioned basis.
    pub fn resolve_outbox_conflict(
        &self,
        id: OutboxId,
        replacement: OutboxPayload,
    ) -> Result<DatabaseRequest<()>> {
        self.outbox_request(|reply| OutboxCommand::ResolveConflict {
            id,
            replacement: Box::new(replacement),
            reply,
        })
    }

    /// Enqueues retry after explicit user resolution of an unknown outcome.
    pub fn resolve_outbox_outcome_unknown(&self, id: OutboxId) -> Result<DatabaseRequest<()>> {
        self.outbox_request(|reply| OutboxCommand::ResolveOutcomeUnknown { id, reply })
    }

    /// Enqueues removal of a terminal Outbox record without deleting Messages.
    pub fn dismiss_outbox(&self, id: OutboxId) -> Result<DatabaseRequest<()>> {
        self.outbox_request(|reply| OutboxCommand::Dismiss { id, reply })
    }

    /// Enqueues cancellation or a cancellation request for nonterminal work.
    pub fn cancel_outbox(&self, id: OutboxId) -> Result<DatabaseRequest<()>> {
        self.outbox_request(|reply| OutboxCommand::Cancel { id, reply })
    }

    /// Enqueues confirmation that cancellation prevented the operation from
    /// being sent.
    pub fn confirm_outbox_unsent(&self, id: OutboxId) -> Result<DatabaseRequest<()>> {
        self.outbox_request(|reply| OutboxCommand::ConfirmUnsent { id, reply })
    }

    /// Enqueues an explicit transition to an unknown remote outcome.
    pub fn mark_outbox_outcome_unknown(
        &self,
        id: OutboxId,
        reason: String,
    ) -> Result<DatabaseRequest<()>> {
        self.outbox_request(|reply| OutboxCommand::MarkOutcomeUnknown { id, reason, reply })
    }

    /// Enqueues an atomic optional Message replacement and acknowledgement.
    pub fn acknowledge_outbox(
        &self,
        id: OutboxId,
        replacement: Option<StoredMessage>,
    ) -> Result<DatabaseRequest<()>> {
        self.outbox_request(|reply| OutboxCommand::Acknowledge {
            id,
            replacement: replacement.map(Box::new),
            reply,
        })
    }
}
