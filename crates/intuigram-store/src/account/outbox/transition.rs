use rusqlite::{Connection, OptionalExtension, params};
use snafu::ResultExt;

use super::repository::{DatabaseSnafu, Error, Result};
use super::{OutboxId, OutboxState, mapping};

pub(super) fn apply(
    connection: &Connection,
    id: OutboxId,
    allowed: &[OutboxState],
    target: OutboxState,
    available_at: Option<i64>,
    last_error: Option<String>,
) -> Result<()> {
    require(connection, id, allowed, target)?;
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

pub(super) fn require(
    connection: &Connection,
    id: OutboxId,
    allowed: &[OutboxState],
    target: OutboxState,
) -> Result<()> {
    let state = current(connection, id)?;
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

pub(super) fn current(connection: &Connection, id: OutboxId) -> Result<OutboxState> {
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
    mapping::parse_state(&state)
}
