use snafu::ResultExt;

use super::super::{cancellation, completion, expiry, lifecycle, repository, resolution};
use super::OutboxCommand;
use crate::account::OutboxSnafu;

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
            reply.finish(expiry::sweep(connection, now).context(OutboxSnafu));
        }
        OutboxCommand::SetExpiry { id, expiry, reply } => {
            reply.finish(expiry::set(connection, id, expiry).context(OutboxSnafu));
        }
        OutboxCommand::Retry { id, reply } => {
            reply.finish(resolution::retry_failed(connection, id).context(OutboxSnafu));
        }
        OutboxCommand::ResolveConflict {
            id,
            replacement,
            reply,
        } => reply.finish(
            resolution::resolve_conflict(connection, id, *replacement).context(OutboxSnafu),
        ),
        OutboxCommand::ResolveOutcomeUnknown { id, reply } => {
            reply.finish(resolution::resolve_outcome_unknown(connection, id).context(OutboxSnafu))
        }
        OutboxCommand::Dismiss { id, reply } => {
            reply.finish(resolution::dismiss(connection, id).context(OutboxSnafu));
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
        OutboxCommand::Complete {
            id,
            completion,
            reply,
        } => reply.finish(completion::finish(connection, id, *completion).context(OutboxSnafu)),
    }
}
