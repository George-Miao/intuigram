use rusqlite::Connection;
use snafu::ResultExt;

use super::repository::{DatabaseSnafu, Error, Result};
use super::{OutboxId, OutboxOperation, OutboxPayload, OutboxState, mapping, transition};
use crate::account::{StoredMessage, StoredMutation, apply_sync_mutation, upsert_message};

/// Normalized durable result committed while completing one Outbox item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutboxCompletion {
    /// Telegram acknowledged the operation without returning durable records.
    Acknowledged,

    /// Telegram returned the normalized Message for a `Create` or `Send`.
    Message(Box<StoredMessage>),

    /// Telegram returned normalized effects for a `Mutation`.
    Mutations(Vec<StoredMutation>),
}

pub(super) fn finish(
    connection: &Connection,
    id: OutboxId,
    completion: OutboxCompletion,
) -> Result<()> {
    let transaction = connection.unchecked_transaction().context(DatabaseSnafu {
        operation: "complete",
    })?;
    transition::require(
        &transaction,
        id,
        &[OutboxState::InFlight, OutboxState::CancelRequested],
        OutboxState::Ready,
    )?;
    let record = mapping::load_one(&transaction, id)?;
    match completion {
        OutboxCompletion::Acknowledged => {}
        OutboxCompletion::Message(message)
            if matches!(
                record.operation,
                OutboxOperation::Create | OutboxOperation::Send
            ) =>
        {
            commit_message(&transaction, record.operation, record.payload, &message)?;
        }
        OutboxCompletion::Mutations(mutations) if record.operation == OutboxOperation::Mutation => {
            for mutation in mutations {
                apply_sync_mutation(&transaction, mutation).context(DatabaseSnafu {
                    operation: "complete",
                })?;
            }
        }
        completion => {
            return Err(Error::CompletionMismatch {
                id,
                operation: record.operation,
                completion: completion.kind(),
            });
        }
    }
    transaction
        .execute("DELETE FROM outbox WHERE outbox_id = ?1", [id.get()])
        .context(DatabaseSnafu {
            operation: "complete",
        })?;
    transaction.commit().context(DatabaseSnafu {
        operation: "complete",
    })
}

impl OutboxCompletion {
    const fn kind(&self) -> &'static str {
        match self {
            Self::Acknowledged => "acknowledgement",
            Self::Message(_) => "Message",
            Self::Mutations(_) => "mutations",
        }
    }
}

fn commit_message(
    connection: &Connection,
    operation: OutboxOperation,
    payload: OutboxPayload,
    message: &StoredMessage,
) -> Result<()> {
    let OutboxPayload::V1(payload) = payload;
    if message.chat_id != payload.chat_id
        || message.thread_root != payload.thread_root
        || message.saved_peer != payload.saved_peer
    {
        return Err(Error::AcknowledgementMessageMismatch);
    }
    if operation == OutboxOperation::Send && payload.local_message_id.is_none() {
        return Err(Error::MissingLocalMessageId);
    }
    upsert_message(connection, message).context(DatabaseSnafu {
        operation: "complete",
    })?;
    if let Some(local_id) = payload.local_message_id
        && local_id != message.id
    {
        apply_sync_mutation(
            connection,
            StoredMutation::DeleteMessages {
                chat_id: Some(payload.chat_id),
                ids: vec![local_id],
            },
        )
        .context(DatabaseSnafu {
            operation: "complete",
        })?;
    }
    Ok(())
}
