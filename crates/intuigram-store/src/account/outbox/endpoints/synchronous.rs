use std::sync::mpsc;

use super::{OutboxCommand, Reply};
use crate::account::worker::Command;
use crate::account::{
    AccountDatabase, OutboxAdmission, OutboxCompletion, OutboxExpiry, OutboxId, OutboxPayload,
    OutboxPoll, OutboxRecord, Result, StoredMessage,
};

impl AccountDatabase {
    fn outbox_request<T>(&self, build: impl FnOnce(Reply<T>) -> OutboxCommand) -> Result<T> {
        let (reply, response) = mpsc::sync_channel(1);
        self.commands
            .send(Command::Outbox(build(Reply::Sync(reply))))
            .map_err(|_| crate::account::Error::WorkerUnavailable)?;
        response
            .recv()
            .map_err(|_| crate::account::Error::WorkerUnavailable)?
    }

    /// Atomically admits durable work, exact media, an optimistic Message, and
    /// scoped Draft consumption.
    pub fn admit_outbox(&self, admission: OutboxAdmission) -> Result<OutboxId> {
        self.outbox_request(|reply| OutboxCommand::Admit {
            admission: Box::new(admission),
            reply,
        })
    }

    /// Loads all durable Outbox items in FIFO order.
    pub fn load_outbox(&self) -> Result<Vec<OutboxRecord>> {
        self.outbox_request(|reply| OutboxCommand::Load { reply })
    }

    /// Polls and possibly claims the single FIFO Outbox head.
    pub fn claim_outbox(&self, now: i64) -> Result<OutboxPoll> {
        self.outbox_request(|reply| OutboxCommand::Claim { now, reply })
    }

    /// Defers claimed work until an explicit retry timestamp.
    pub fn defer_outbox(&self, id: OutboxId, available_at: i64, reason: String) -> Result<()> {
        self.outbox_request(|reply| OutboxCommand::Defer {
            id,
            available_at,
            reason,
            reply,
        })
    }

    /// Marks claimed work permanently failed.
    pub fn fail_outbox(&self, id: OutboxId, reason: String) -> Result<()> {
        self.outbox_request(|reply| OutboxCommand::Fail { id, reason, reply })
    }

    /// Marks claimed replay-unsafe work as requiring resolution.
    pub fn conflict_outbox(&self, id: OutboxId, reason: String) -> Result<()> {
        self.outbox_request(|reply| OutboxCommand::Conflict { id, reason, reply })
    }

    /// Expires explicitly time-bounded, unclaimed work.
    pub fn expire_outbox(&self, now: i64) -> Result<Vec<OutboxId>> {
        self.outbox_request(|reply| OutboxCommand::Expire { now, reply })
    }

    /// Sets or clears a caller-chosen deadline while work is not active.
    pub fn set_outbox_expiry(&self, id: OutboxId, expiry: OutboxExpiry) -> Result<()> {
        self.outbox_request(|reply| OutboxCommand::SetExpiry { id, expiry, reply })
    }

    /// Returns replay-safe failed work to FIFO execution.
    pub fn retry_outbox(&self, id: OutboxId) -> Result<()> {
        self.outbox_request(|reply| OutboxCommand::Retry { id, reply })
    }

    /// Replaces a conflicted operation's versioned basis and retries it.
    pub fn resolve_outbox_conflict(&self, id: OutboxId, replacement: OutboxPayload) -> Result<()> {
        self.outbox_request(|reply| OutboxCommand::ResolveConflict {
            id,
            replacement: Box::new(replacement),
            reply,
        })
    }

    /// Retries an unknown remote outcome after explicit user resolution.
    pub fn resolve_outbox_outcome_unknown(&self, id: OutboxId) -> Result<()> {
        self.outbox_request(|reply| OutboxCommand::ResolveOutcomeUnknown { id, reply })
    }

    /// Removes a terminal Outbox record while retaining local Messages.
    pub fn dismiss_outbox(&self, id: OutboxId) -> Result<()> {
        self.outbox_request(|reply| OutboxCommand::Dismiss { id, reply })
    }

    /// Cancels unstarted work or requests cancellation of in-flight work.
    pub fn cancel_outbox(&self, id: OutboxId) -> Result<()> {
        self.outbox_request(|reply| OutboxCommand::Cancel { id, reply })
    }

    /// Confirms that cancellation prevented the operation from being sent.
    pub fn confirm_outbox_unsent(&self, id: OutboxId) -> Result<()> {
        self.outbox_request(|reply| OutboxCommand::ConfirmUnsent { id, reply })
    }

    /// Records that the adapter cannot determine the operation's remote
    /// outcome.
    pub fn mark_outbox_outcome_unknown(&self, id: OutboxId, reason: String) -> Result<()> {
        self.outbox_request(|reply| OutboxCommand::MarkOutcomeUnknown { id, reason, reply })
    }

    /// Atomically commits a normalized durable result and removes active work.
    pub fn complete_outbox(&self, id: OutboxId, completion: OutboxCompletion) -> Result<()> {
        self.outbox_request(|reply| OutboxCommand::Complete {
            id,
            completion: Box::new(completion),
            reply,
        })
    }

    /// Atomically installs an optional normalized server Message and removes
    /// its local optimistic row, acknowledged work, and retained media.
    pub fn acknowledge_outbox(
        &self,
        id: OutboxId,
        replacement: Option<StoredMessage>,
    ) -> Result<()> {
        self.complete_outbox(
            id,
            replacement.map_or(OutboxCompletion::Acknowledged, |message| {
                OutboxCompletion::Message(Box::new(message))
            }),
        )
    }
}
