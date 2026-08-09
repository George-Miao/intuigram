use rusqlite::{Connection, OptionalExtension};
use snafu::ResultExt;

use super::repository::{DatabaseSnafu, Error, Result};
use super::{OutboxId, OutboxPayload, OutboxPoll, OutboxState, expiry, mapping, transition};
use crate::account::{StoredMessage, replace_message_in};

pub(super) fn claim(connection: &Connection, now: i64) -> Result<OutboxPoll> {
    let transaction = connection
        .unchecked_transaction()
        .context(DatabaseSnafu { operation: "claim" })?;
    let active = transaction
        .query_row(
            "SELECT outbox_id FROM outbox WHERE state IN ('in_flight', 'cancel_requested') ORDER \
             BY admitted_at, outbox_id LIMIT 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .context(DatabaseSnafu { operation: "claim" })?;
    if let Some(raw_id) = active {
        let id = OutboxId::from_stored(raw_id).ok_or(Error::InvalidId { value: raw_id })?;
        transaction
            .commit()
            .context(DatabaseSnafu { operation: "claim" })?;
        return Ok(OutboxPoll::Busy { id });
    }
    expiry::sweep_in(&transaction, now)?;
    let head = transaction
        .query_row(
            "SELECT outbox_id, state, available_at FROM outbox WHERE state IN ('ready', \
             'deferred') ORDER BY admitted_at, outbox_id LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                ))
            },
        )
        .optional()
        .context(DatabaseSnafu { operation: "claim" })?;
    let Some((raw_id, state, available_at)) = head else {
        transaction
            .commit()
            .context(DatabaseSnafu { operation: "claim" })?;
        return Ok(OutboxPoll::Idle);
    };
    let id = OutboxId::from_stored(raw_id).ok_or(Error::InvalidId { value: raw_id })?;
    if mapping::parse_state(&state)? == OutboxState::Deferred {
        let available_at = available_at.ok_or(Error::MissingAvailability { id })?;
        if available_at > now {
            transaction
                .commit()
                .context(DatabaseSnafu { operation: "claim" })?;
            return Ok(OutboxPoll::WaitingUntil { id, available_at });
        }
    }
    transaction
        .execute(
            "UPDATE outbox SET state = 'in_flight', available_at = NULL, attempts = attempts + 1 \
             WHERE outbox_id = ?1",
            [raw_id],
        )
        .context(DatabaseSnafu { operation: "claim" })?;
    let record = mapping::load_one(&transaction, id)?;
    transaction
        .commit()
        .context(DatabaseSnafu { operation: "claim" })?;
    Ok(OutboxPoll::Claimed(record))
}

pub(super) fn defer(
    connection: &Connection,
    id: OutboxId,
    available_at: i64,
    reason: String,
) -> Result<()> {
    transition::apply(
        connection,
        id,
        &[OutboxState::InFlight],
        OutboxState::Deferred,
        Some(available_at),
        Some(reason),
    )
}

pub(super) fn fail(connection: &Connection, id: OutboxId, reason: String) -> Result<()> {
    transition::apply(
        connection,
        id,
        &[OutboxState::InFlight],
        OutboxState::Failed,
        None,
        Some(reason),
    )
}

pub(super) fn conflict(connection: &Connection, id: OutboxId, reason: String) -> Result<()> {
    transition::apply(
        connection,
        id,
        &[OutboxState::InFlight],
        OutboxState::Conflict,
        None,
        Some(reason),
    )
}

pub(super) fn acknowledge(
    connection: &Connection,
    id: OutboxId,
    replacement: Option<StoredMessage>,
) -> Result<()> {
    let transaction = connection.unchecked_transaction().context(DatabaseSnafu {
        operation: "acknowledge",
    })?;
    transition::require(
        &transaction,
        id,
        &[OutboxState::InFlight, OutboxState::CancelRequested],
        OutboxState::Ready,
    )?;
    if let Some(replacement) = replacement {
        replace_acknowledged_message(&transaction, id, &replacement)?;
    }
    transaction
        .execute("DELETE FROM outbox WHERE outbox_id = ?1", [id.get()])
        .context(DatabaseSnafu {
            operation: "acknowledge",
        })?;
    transaction.commit().context(DatabaseSnafu {
        operation: "acknowledge",
    })
}

fn replace_acknowledged_message(
    connection: &Connection,
    id: OutboxId,
    replacement: &StoredMessage,
) -> Result<()> {
    let record = mapping::load_one(connection, id)?;
    let OutboxPayload::V1(payload) = record.payload;
    let local_id = payload
        .local_message_id
        .ok_or(Error::MissingLocalMessageId)?;
    if replacement.chat_id != payload.chat_id
        || replacement.thread_root != payload.thread_root
        || replacement.saved_peer != payload.saved_peer
    {
        return Err(Error::AcknowledgementMessageMismatch);
    }
    replace_message_in(connection, payload.chat_id, local_id, replacement).context(DatabaseSnafu {
        operation: "acknowledge",
    })
}
