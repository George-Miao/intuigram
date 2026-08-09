use rusqlite::{Connection, params};
use snafu::ResultExt;

use super::repository::{DatabaseSnafu, Error, PayloadSnafu, Result};
use super::{OutboxId, OutboxMedia, OutboxOperation, OutboxRecord, OutboxState, codec};

pub(super) fn load(connection: &Connection) -> Result<Vec<OutboxRecord>> {
    let mut statement = connection
        .prepare(
            "SELECT outbox_id, operation, state, payload, admitted_at, available_at, expires_at, \
             attempts, last_error FROM outbox ORDER BY admitted_at, outbox_id",
        )
        .context(DatabaseSnafu { operation: "load" })?;
    let rows = statement
        .query_map([], |row| {
            Ok(StoredRow {
                id: row.get(0)?,
                operation: row.get(1)?,
                state: row.get(2)?,
                payload: row.get(3)?,
                admitted_at: row.get(4)?,
                available_at: row.get(5)?,
                expires_at: row.get(6)?,
                attempts: row.get(7)?,
                last_error: row.get(8)?,
            })
        })
        .context(DatabaseSnafu { operation: "load" })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context(DatabaseSnafu { operation: "load" })?;
    rows.into_iter()
        .map(|row| decode_row(connection, row))
        .collect()
}

pub(super) fn load_one(connection: &Connection, id: OutboxId) -> Result<OutboxRecord> {
    load(connection)?
        .into_iter()
        .find(|record| record.id == id)
        .ok_or(Error::NotFound { id })
}

fn decode_row(connection: &Connection, row: StoredRow) -> Result<OutboxRecord> {
    let id = OutboxId::from_stored(row.id).ok_or(Error::InvalidId { value: row.id })?;
    let attempts = u32::try_from(row.attempts).map_err(|_| Error::InvalidValue {
        column: "attempts",
        value: row.attempts.to_string(),
    })?;
    Ok(OutboxRecord {
        id,
        operation: parse_operation(&row.operation)?,
        state: parse_state(&row.state)?,
        payload: codec::decode(&row.payload).context(PayloadSnafu)?,
        media: load_media(connection, id)?,
        admitted_at: row.admitted_at,
        available_at: row.available_at,
        expires_at: row.expires_at,
        attempts,
        last_error: row.last_error,
    })
}

pub(super) fn insert_media(
    connection: &Connection,
    id: OutboxId,
    media: &[OutboxMedia],
) -> Result<()> {
    for (position, media) in media.iter().enumerate() {
        connection
            .execute(
                "INSERT INTO outbox_media(outbox_id, position, file_name, mime_type, bytes, \
                 sha256) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    id.get(),
                    position as i64,
                    media.file_name,
                    media.mime_type,
                    media.bytes,
                    media.sha256
                ],
            )
            .context(DatabaseSnafu {
                operation: "retain media",
            })?;
    }
    Ok(())
}

fn load_media(connection: &Connection, id: OutboxId) -> Result<Vec<OutboxMedia>> {
    let mut statement = connection
        .prepare(
            "SELECT file_name, mime_type, bytes, sha256 FROM outbox_media WHERE outbox_id = ?1 \
             ORDER BY position",
        )
        .context(DatabaseSnafu {
            operation: "load media",
        })?;
    let rows = statement
        .query_map([id.get()], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get::<_, Vec<u8>>(3)?,
            ))
        })
        .context(DatabaseSnafu {
            operation: "load media",
        })?
        .collect::<std::result::Result<Vec<(String, String, Vec<u8>, Vec<u8>)>, _>>()
        .context(DatabaseSnafu {
            operation: "load media",
        })?;
    rows.into_iter()
        .enumerate()
        .map(|(position, (file_name, mime_type, bytes, hash))| {
            let sha256: [u8; 32] = hash
                .try_into()
                .map_err(|_| Error::InvalidMediaHash { id, position })?;
            let media = OutboxMedia {
                file_name,
                mime_type,
                bytes,
                sha256,
            };
            if media.hash_is_valid() {
                Ok(media)
            } else {
                Err(Error::InvalidMediaHash { id, position })
            }
        })
        .collect()
}

pub(super) fn operation_name(operation: OutboxOperation) -> &'static str {
    match operation {
        OutboxOperation::Create => "create",
        OutboxOperation::Send => "send",
        OutboxOperation::Mutation => "mutation",
    }
}

fn parse_operation(value: &str) -> Result<OutboxOperation> {
    match value {
        "create" => Ok(OutboxOperation::Create),
        "send" => Ok(OutboxOperation::Send),
        "mutation" => Ok(OutboxOperation::Mutation),
        value => Err(Error::InvalidValue {
            column: "operation",
            value: value.to_owned(),
        }),
    }
}

pub(super) fn state_name(state: OutboxState) -> &'static str {
    match state {
        OutboxState::Ready => "ready",
        OutboxState::InFlight => "in_flight",
        OutboxState::CancelRequested => "cancel_requested",
        OutboxState::Deferred => "deferred",
        OutboxState::Failed => "failed",
        OutboxState::Conflict => "conflict",
        OutboxState::OutcomeUnknown => "outcome_unknown",
        OutboxState::Expired => "expired",
        OutboxState::Cancelled => "cancelled",
    }
}

pub(super) fn parse_state(value: &str) -> Result<OutboxState> {
    match value {
        "ready" => Ok(OutboxState::Ready),
        "in_flight" => Ok(OutboxState::InFlight),
        "cancel_requested" => Ok(OutboxState::CancelRequested),
        "deferred" => Ok(OutboxState::Deferred),
        "failed" => Ok(OutboxState::Failed),
        "conflict" => Ok(OutboxState::Conflict),
        "outcome_unknown" => Ok(OutboxState::OutcomeUnknown),
        "expired" => Ok(OutboxState::Expired),
        "cancelled" => Ok(OutboxState::Cancelled),
        value => Err(Error::InvalidValue {
            column: "state",
            value: value.to_owned(),
        }),
    }
}

struct StoredRow {
    id: i64,
    operation: String,
    state: String,
    payload: Vec<u8>,
    admitted_at: i64,
    available_at: Option<i64>,
    expires_at: Option<i64>,
    attempts: i64,
    last_error: Option<String>,
}
