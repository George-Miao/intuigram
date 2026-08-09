mod asynchronous;
mod dispatch;
mod synchronous;

use std::sync::mpsc::SyncSender;

pub(in crate::account) use dispatch::execute;

use super::{
    OutboxAdmission, OutboxCompletion, OutboxExpiry, OutboxId, OutboxPayload, OutboxPoll,
    OutboxRecord,
};
use crate::account::Result;
use crate::account::worker::AsyncReply;

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
    SetExpiry {
        id: OutboxId,
        expiry: OutboxExpiry,
        reply: Reply<()>,
    },
    Retry {
        id: OutboxId,
        reply: Reply<()>,
    },
    ResolveConflict {
        id: OutboxId,
        replacement: Box<OutboxPayload>,
        reply: Reply<()>,
    },
    ResolveOutcomeUnknown {
        id: OutboxId,
        reply: Reply<()>,
    },
    Dismiss {
        id: OutboxId,
        reply: Reply<()>,
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
    Complete {
        id: OutboxId,
        completion: Box<OutboxCompletion>,
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
