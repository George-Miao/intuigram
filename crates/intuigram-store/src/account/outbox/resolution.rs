use rusqlite::{Connection, params};
use snafu::ResultExt;

use super::repository::{DatabaseSnafu, Error, PayloadSnafu, Result};
use super::{OutboxId, OutboxOperation, OutboxPayload, OutboxState, codec, mapping, transition};

pub(super) fn retry_failed(connection: &Connection, id: OutboxId) -> Result<()> {
    let record = mapping::load_one(connection, id)?;
    transition::require(connection, id, &[OutboxState::Failed], OutboxState::Ready)?;
    if record.operation == OutboxOperation::Mutation {
        return Err(Error::UnsafeRetry {
            id,
            operation: record.operation,
        });
    }
    transition::apply(
        connection,
        id,
        &[OutboxState::Failed],
        OutboxState::Ready,
        None,
        None,
    )
}

pub(super) fn resolve_conflict(
    connection: &Connection,
    id: OutboxId,
    replacement: OutboxPayload,
) -> Result<()> {
    let payload = codec::encode(&replacement).context(PayloadSnafu)?;
    let changed = connection
        .execute(
            "UPDATE outbox SET state = 'ready', payload = ?2, available_at = NULL, last_error = \
             NULL WHERE outbox_id = ?1 AND state = 'conflict'",
            params![id.get(), payload],
        )
        .context(DatabaseSnafu {
            operation: "resolve conflict",
        })?;
    if changed == 1 {
        Ok(())
    } else {
        transition::require(connection, id, &[OutboxState::Conflict], OutboxState::Ready)
    }
}

pub(super) fn resolve_outcome_unknown(connection: &Connection, id: OutboxId) -> Result<()> {
    transition::apply(
        connection,
        id,
        &[OutboxState::OutcomeUnknown],
        OutboxState::Ready,
        None,
        None,
    )
}

pub(super) fn dismiss(connection: &Connection, id: OutboxId) -> Result<()> {
    let changed = connection
        .execute(
            "DELETE FROM outbox WHERE outbox_id = ?1 AND state IN ('failed', 'conflict', \
             'outcome_unknown', 'expired', 'cancelled')",
            [id.get()],
        )
        .context(DatabaseSnafu {
            operation: "dismiss",
        })?;
    if changed == 1 {
        Ok(())
    } else {
        Err(Error::NotDismissible {
            id,
            state: transition::current(connection, id)?,
        })
    }
}
