use std::sync::mpsc;

use super::{OutboxCommand, Reply};
use crate::account::worker::Command;
use crate::account::{
    AccountDatabase, OutboxAdmission, OutboxId, OutboxPoll, OutboxRecord, Result, StoredMessage,
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

    /// Atomically installs an optional normalized server Message and removes
    /// its local optimistic row, acknowledged work, and retained media.
    pub fn acknowledge_outbox(
        &self,
        id: OutboxId,
        replacement: Option<StoredMessage>,
    ) -> Result<()> {
        self.outbox_request(|reply| OutboxCommand::Acknowledge {
            id,
            replacement: replacement.map(Box::new),
            reply,
        })
    }
}
