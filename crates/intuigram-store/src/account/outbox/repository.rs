use rusqlite::{Connection, params};
use snafu::{ResultExt, Snafu};

use super::{
    OutboxAdmission, OutboxExpiry, OutboxId, OutboxOperation, OutboxPayload, OutboxRecord,
    OutboxState, codec, mapping,
};
use crate::account::upsert_message;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(super)))]
pub enum Error {
    #[snafu(display("failed to {operation} durable Outbox records"))]
    Database {
        operation: &'static str,
        source: rusqlite::Error,
    },

    #[snafu(display("durable Outbox payload could not be decoded"))]
    Payload { source: codec::Error },

    #[snafu(display("durable Outbox row has invalid ID {value}"))]
    InvalidId { value: i64 },

    #[snafu(display("durable Outbox row has invalid {column} value {value}"))]
    InvalidValue { column: &'static str, value: String },

    #[snafu(display("Outbox item {} does not exist", id.get()))]
    NotFound { id: OutboxId },

    #[snafu(display("deferred Outbox item {} has no availability timestamp", id.get()))]
    MissingAvailability { id: OutboxId },

    #[snafu(display(
        "Outbox item {} cannot transition from {from:?} to {to:?}",
        id.get()
    ))]
    InvalidTransition {
        id: OutboxId,
        from: OutboxState,
        to: OutboxState,
    },

    #[snafu(display("Outbox item {} in state {state:?} cannot change expiry", id.get()))]
    ExpiryNotEditable { id: OutboxId, state: OutboxState },

    #[snafu(display("Outbox item {} with operation {operation:?} is not safe to retry", id.get()))]
    UnsafeRetry {
        id: OutboxId,
        operation: OutboxOperation,
    },

    #[snafu(display("Outbox item {} in state {state:?} cannot be dismissed", id.get()))]
    NotDismissible { id: OutboxId, state: OutboxState },

    #[snafu(display(
        "Outbox item {} with operation {operation:?} cannot commit {completion}",
        id.get()
    ))]
    CompletionMismatch {
        id: OutboxId,
        operation: OutboxOperation,
        completion: &'static str,
    },

    #[snafu(display("Outbox optimistic Message does not match its payload scope"))]
    OptimisticMessageMismatch,

    #[snafu(display("Outbox acknowledgement Message does not match its payload scope"))]
    AcknowledgementMessageMismatch,

    #[snafu(display("Outbox acknowledgement requires a local Message identity"))]
    MissingLocalMessageId,

    #[snafu(display("Outbox media {position} for item {} has an invalid hash", id.get()))]
    InvalidMediaHash { id: OutboxId, position: usize },

    #[snafu(display("Outbox admission media {position} has an invalid hash"))]
    InvalidAdmissionMediaHash { position: usize },
}

pub type Result<T> = std::result::Result<T, Error>;

pub(super) fn admit(connection: &Connection, admission: OutboxAdmission) -> Result<OutboxId> {
    validate_admission(&admission)?;
    let payload = codec::encode(&admission.payload).context(PayloadSnafu)?;
    let expires_at = match admission.expiry {
        OutboxExpiry::Never => None,
        OutboxExpiry::At(timestamp) => Some(timestamp),
    };
    let transaction = connection
        .unchecked_transaction()
        .context(DatabaseSnafu { operation: "admit" })?;
    transaction
        .execute(
            "INSERT INTO outbox(operation, state, payload, admitted_at, expires_at) VALUES (?1, \
             'ready', ?2, ?3, ?4)",
            params![
                mapping::operation_name(admission.operation),
                payload,
                admission.admitted_at,
                expires_at
            ],
        )
        .context(DatabaseSnafu { operation: "admit" })?;
    let raw_id = transaction.last_insert_rowid();
    let id = OutboxId::from_stored(raw_id).ok_or(Error::InvalidId { value: raw_id })?;
    mapping::insert_media(&transaction, id, &admission.media)?;
    if let Some(message) = &admission.optimistic_message {
        upsert_message(&transaction, message).context(DatabaseSnafu { operation: "admit" })?;
    }
    if admission.consume_draft {
        let (chat, thread, saved_peer) = admission.payload.scope();
        transaction
            .execute(
                "DELETE FROM drafts WHERE chat_id = ?1 AND thread_root_message_id = ?2 AND \
                 saved_peer_id = ?3",
                params![chat, thread.unwrap_or(0), saved_peer.unwrap_or(0)],
            )
            .context(DatabaseSnafu { operation: "admit" })?;
    }
    transaction
        .commit()
        .context(DatabaseSnafu { operation: "admit" })?;
    Ok(id)
}

fn validate_admission(admission: &OutboxAdmission) -> Result<()> {
    if let Some(message) = &admission.optimistic_message {
        let OutboxPayload::V1(payload) = &admission.payload;
        if message.chat_id != payload.chat_id
            || message.thread_root != payload.thread_root
            || message.saved_peer != payload.saved_peer
            || Some(message.id) != payload.local_message_id
        {
            return Err(Error::OptimisticMessageMismatch);
        }
    }
    for (position, media) in admission.media.iter().enumerate() {
        if !media.hash_is_valid() {
            return Err(Error::InvalidAdmissionMediaHash { position });
        }
    }
    Ok(())
}

pub(crate) fn load(connection: &Connection) -> Result<Vec<OutboxRecord>> {
    mapping::load(connection)
}

pub(crate) fn restore(connection: &Connection, records: &[OutboxRecord]) -> Result<()> {
    for record in records {
        let payload = codec::encode(&record.payload).context(PayloadSnafu)?;
        connection
            .execute(
                "INSERT INTO outbox(outbox_id, operation, state, payload, admitted_at, \
                 available_at, expires_at, attempts, last_error) VALUES (?1, ?2, ?3, ?4, ?5, ?6, \
                 ?7, ?8, ?9)",
                params![
                    record.id.get(),
                    mapping::operation_name(record.operation),
                    mapping::state_name(record.state),
                    payload,
                    record.admitted_at,
                    record.available_at,
                    record.expires_at,
                    record.attempts,
                    record.last_error
                ],
            )
            .context(DatabaseSnafu {
                operation: "restore",
            })?;
        mapping::insert_media(connection, record.id, &record.media)?;
    }
    Ok(())
}
