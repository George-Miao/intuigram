use rusqlite::{Connection, OptionalExtension, params};
use snafu::ResultExt;

use super::repository::{DatabaseSnafu, Error, Result};
use super::{OutboxId, OutboxPayload, OutboxRecord, OutboxState, mapping, repository};
use crate::account::{StoredMessage, replace_message_in};

pub(super) fn claim(connection: &Connection, now: i64) -> Result<Option<OutboxRecord>> {
    let transaction = connection
        .unchecked_transaction()
        .context(DatabaseSnafu { operation: "claim" })?;
    let raw_id = transaction
        .query_row(
            "SELECT outbox_id FROM outbox WHERE (state = 'ready' OR (state = 'deferred' AND \
             available_at <= ?1)) AND (expires_at IS NULL OR expires_at > ?1) ORDER BY \
             admitted_at, outbox_id LIMIT 1",
            [now],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .context(DatabaseSnafu { operation: "claim" })?;
    let Some(raw_id) = raw_id else {
        transaction
            .commit()
            .context(DatabaseSnafu { operation: "claim" })?;
        return Ok(None);
    };
    transaction
        .execute(
            "UPDATE outbox SET state = 'in_flight', available_at = NULL, attempts = attempts + 1 \
             WHERE outbox_id = ?1",
            [raw_id],
        )
        .context(DatabaseSnafu { operation: "claim" })?;
    transaction
        .commit()
        .context(DatabaseSnafu { operation: "claim" })?;
    let id = OutboxId::from_stored(raw_id).ok_or(Error::InvalidId { value: raw_id })?;
    mapping::load_one(connection, id).map(Some)
}

pub(super) fn defer(
    connection: &Connection,
    id: OutboxId,
    available_at: i64,
    reason: String,
) -> Result<()> {
    transition(
        connection,
        id,
        &[OutboxState::InFlight],
        OutboxState::Deferred,
        Some(available_at),
        Some(reason),
    )
}

pub(super) fn fail(connection: &Connection, id: OutboxId, reason: String) -> Result<()> {
    transition(
        connection,
        id,
        &[OutboxState::InFlight],
        OutboxState::Failed,
        None,
        Some(reason),
    )
}

pub(super) fn conflict(connection: &Connection, id: OutboxId, reason: String) -> Result<()> {
    transition(
        connection,
        id,
        &[OutboxState::InFlight],
        OutboxState::Conflict,
        None,
        Some(reason),
    )
}

pub(super) fn cancel(connection: &Connection, id: OutboxId) -> Result<()> {
    transition(
        connection,
        id,
        &[
            OutboxState::Ready,
            OutboxState::Deferred,
            OutboxState::InFlight,
        ],
        OutboxState::Cancelled,
        None,
        None,
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
    require_state(
        &transaction,
        id,
        &[OutboxState::InFlight],
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

pub(super) fn expire(connection: &Connection, now: i64) -> Result<Vec<OutboxId>> {
    let ids = repository::load(connection)?
        .into_iter()
        .filter(|record| {
            matches!(record.state, OutboxState::Ready | OutboxState::Deferred)
                && record.expires_at.is_some_and(|expiry| expiry <= now)
        })
        .map(|record| record.id)
        .collect::<Vec<_>>();
    let transaction = connection.unchecked_transaction().context(DatabaseSnafu {
        operation: "expire",
    })?;
    for id in &ids {
        transaction
            .execute(
                "UPDATE outbox SET state = 'expired', available_at = NULL WHERE outbox_id = ?1",
                [id.get()],
            )
            .context(DatabaseSnafu {
                operation: "expire",
            })?;
    }
    transaction.commit().context(DatabaseSnafu {
        operation: "expire",
    })?;
    Ok(ids)
}

pub(in crate::account) fn recover_in_flight(connection: &Connection) -> Result<()> {
    let transaction = connection.unchecked_transaction().context(DatabaseSnafu {
        operation: "recover",
    })?;
    transaction
        .execute(
            "UPDATE outbox SET state = 'ready', available_at = NULL WHERE state = 'in_flight' AND \
             operation IN ('create', 'send')",
            [],
        )
        .context(DatabaseSnafu {
            operation: "recover",
        })?;
    transaction
        .execute(
            "UPDATE outbox SET state = 'conflict', available_at = NULL, last_error = 'interrupted \
             before mutation acknowledgement' WHERE state = 'in_flight' AND operation = 'mutation'",
            [],
        )
        .context(DatabaseSnafu {
            operation: "recover",
        })?;
    transaction.commit().context(DatabaseSnafu {
        operation: "recover",
    })
}

fn transition(
    connection: &Connection,
    id: OutboxId,
    allowed: &[OutboxState],
    target: OutboxState,
    available_at: Option<i64>,
    last_error: Option<String>,
) -> Result<()> {
    require_state(connection, id, allowed, target)?;
    connection
        .execute(
            "UPDATE outbox SET state = ?2, available_at = ?3, last_error = ?4 WHERE outbox_id = ?1",
            params![
                id.get(),
                mapping::state_name(target),
                available_at,
                last_error
            ],
        )
        .context(DatabaseSnafu {
            operation: "transition",
        })?;
    Ok(())
}

fn require_state(
    connection: &Connection,
    id: OutboxId,
    allowed: &[OutboxState],
    target: OutboxState,
) -> Result<()> {
    let state = connection
        .query_row(
            "SELECT state FROM outbox WHERE outbox_id = ?1",
            [id.get()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .context(DatabaseSnafu {
            operation: "inspect state",
        })?
        .ok_or(Error::NotFound { id })?;
    let state = mapping::parse_state(&state)?;
    if allowed.contains(&state) {
        Ok(())
    } else {
        Err(Error::InvalidTransition {
            id,
            from: state,
            to: target,
        })
    }
}
