use std::sync::mpsc::{self, SyncSender};

use snafu::ResultExt;

use super::{OutboxAdmission, OutboxId, OutboxPoll, OutboxRecord, lifecycle, repository};
use crate::account::worker::{AsyncReply, Command, async_response, map_try_send_error};
use crate::account::{
    AccountDatabase, AccountStore, DatabaseRequest, OutboxSnafu, Result, StoredMessage,
};

pub(in crate::account) enum OutboxCommand {
    Admit {
        admission: Box<OutboxAdmission>,
        reply: Reply<OutboxId>,
    },
    Load {
        reply: Reply<Vec<OutboxRecord>>,
    },
    Claim {
        now: i64,
        reply: Reply<OutboxPoll>,
    },
    Defer {
        id: OutboxId,
        available_at: i64,
        reason: String,
        reply: Reply<()>,
    },
    Fail {
        id: OutboxId,
        reason: String,
        reply: Reply<()>,
    },
    Conflict {
        id: OutboxId,
        reason: String,
        reply: Reply<()>,
    },
    Expire {
        now: i64,
        reply: Reply<Vec<OutboxId>>,
    },
    Cancel {
        id: OutboxId,
        reply: Reply<()>,
    },
    Acknowledge {
        id: OutboxId,
        replacement: Option<Box<StoredMessage>>,
        reply: Reply<()>,
    },
}

pub(in crate::account) enum Reply<T> {
    Sync(SyncSender<Result<T>>),
    Async(AsyncReply<T>),
}

impl<T> Reply<T> {
    fn finish(self, result: Result<T>) {
        match self {
            Self::Sync(reply) => {
                let _ = reply.send(result);
            }
            Self::Async(reply) => reply.finish(result),
        }
    }
}

pub(in crate::account) fn execute(connection: &rusqlite::Connection, command: OutboxCommand) {
    match command {
        OutboxCommand::Admit { admission, reply } => {
            reply.finish(repository::admit(connection, *admission).context(OutboxSnafu))
        }
        OutboxCommand::Load { reply } => {
            reply.finish(repository::load(connection).context(OutboxSnafu));
        }
        OutboxCommand::Claim { now, reply } => {
            reply.finish(lifecycle::claim(connection, now).context(OutboxSnafu));
        }
        OutboxCommand::Defer {
            id,
            available_at,
            reason,
            reply,
        } => reply
            .finish(lifecycle::defer(connection, id, available_at, reason).context(OutboxSnafu)),
        OutboxCommand::Fail { id, reason, reply } => {
            reply.finish(lifecycle::fail(connection, id, reason).context(OutboxSnafu));
        }
        OutboxCommand::Conflict { id, reason, reply } => {
            reply.finish(lifecycle::conflict(connection, id, reason).context(OutboxSnafu))
        }
        OutboxCommand::Expire { now, reply } => {
            reply.finish(lifecycle::expire(connection, now).context(OutboxSnafu));
        }
        OutboxCommand::Cancel { id, reply } => {
            reply.finish(lifecycle::cancel(connection, id).context(OutboxSnafu));
        }
        OutboxCommand::Acknowledge {
            id,
            replacement,
            reply,
        } => reply.finish(
            lifecycle::acknowledge(connection, id, replacement.map(|message| *message))
                .context(OutboxSnafu),
        ),
    }
}

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

    /// Cancels work that has not reached a terminal state.
    pub fn cancel_outbox(&self, id: OutboxId) -> Result<()> {
        self.outbox_request(|reply| OutboxCommand::Cancel { id, reply })
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

    /// Enqueues cancellation of nonterminal work.
    pub fn cancel_outbox(&self, id: OutboxId) -> Result<DatabaseRequest<()>> {
        self.outbox_request(|reply| OutboxCommand::Cancel { id, reply })
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
