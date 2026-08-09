mod asynchronous;
mod synchronous;

use std::sync::mpsc::SyncSender;

use snafu::ResultExt;

use super::{
    OutboxAdmission, OutboxId, OutboxPoll, OutboxRecord, cancellation, lifecycle, repository,
};
use crate::account::worker::AsyncReply;
use crate::account::{OutboxSnafu, Result, StoredMessage};

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
    ConfirmUnsent {
        id: OutboxId,
        reply: Reply<()>,
    },
    MarkOutcomeUnknown {
        id: OutboxId,
        reason: String,
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
            reply.finish(cancellation::request(connection, id).context(OutboxSnafu));
        }
        OutboxCommand::ConfirmUnsent { id, reply } => {
            reply.finish(cancellation::confirm_unsent(connection, id).context(OutboxSnafu));
        }
        OutboxCommand::MarkOutcomeUnknown { id, reason, reply } => reply.finish(
            cancellation::mark_outcome_unknown(connection, id, reason).context(OutboxSnafu),
        ),
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
