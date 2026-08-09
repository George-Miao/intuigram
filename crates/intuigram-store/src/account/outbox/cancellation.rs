use rusqlite::Connection;

use super::repository::{Error, Result};
use super::{OutboxId, OutboxState, transition};

pub(super) fn request(connection: &Connection, id: OutboxId) -> Result<()> {
    match transition::current(connection, id)? {
        OutboxState::Ready | OutboxState::Deferred => transition::apply(
            connection,
            id,
            &[OutboxState::Ready, OutboxState::Deferred],
            OutboxState::Cancelled,
            None,
            None,
        ),
        OutboxState::InFlight => transition::apply(
            connection,
            id,
            &[OutboxState::InFlight],
            OutboxState::CancelRequested,
            None,
            None,
        ),
        from => Err(Error::InvalidTransition {
            id,
            from,
            to: OutboxState::Cancelled,
        }),
    }
}

pub(super) fn confirm_unsent(connection: &Connection, id: OutboxId) -> Result<()> {
    transition::apply(
        connection,
        id,
        &[OutboxState::CancelRequested],
        OutboxState::Cancelled,
        None,
        None,
    )
}

pub(super) fn mark_outcome_unknown(
    connection: &Connection,
    id: OutboxId,
    reason: String,
) -> Result<()> {
    transition::apply(
        connection,
        id,
        &[OutboxState::InFlight, OutboxState::CancelRequested],
        OutboxState::OutcomeUnknown,
        None,
        Some(reason),
    )
}
