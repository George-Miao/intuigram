use rusqlite::{Connection, params};
use snafu::ResultExt;

use super::repository::{DatabaseSnafu, Error, Result};
use super::{OutboxExpiry, OutboxId, transition};

pub(super) fn set(connection: &Connection, id: OutboxId, expiry: OutboxExpiry) -> Result<()> {
    let expires_at = match expiry {
        OutboxExpiry::Never => None,
        OutboxExpiry::At(timestamp) => Some(timestamp),
    };
    let changed = connection
        .execute(
            "UPDATE outbox SET expires_at = ?2 WHERE outbox_id = ?1 AND state IN ('ready', \
             'deferred', 'failed', 'conflict', 'outcome_unknown')",
            params![id.get(), expires_at],
        )
        .context(DatabaseSnafu {
            operation: "set expiry",
        })?;
    if changed == 1 {
        Ok(())
    } else {
        Err(Error::ExpiryNotEditable {
            id,
            state: transition::current(connection, id)?,
        })
    }
}

pub(super) fn sweep(connection: &Connection, now: i64) -> Result<Vec<OutboxId>> {
    let transaction = connection.unchecked_transaction().context(DatabaseSnafu {
        operation: "expire",
    })?;
    let ids = sweep_in(&transaction, now)?;
    transaction.commit().context(DatabaseSnafu {
        operation: "expire",
    })?;
    Ok(ids)
}

pub(super) fn sweep_in(connection: &Connection, now: i64) -> Result<Vec<OutboxId>> {
    let ids = {
        let mut statement = connection
            .prepare(
                "SELECT outbox_id FROM outbox WHERE state IN ('ready', 'deferred') AND expires_at \
                 <= ?1 ORDER BY admitted_at, outbox_id",
            )
            .context(DatabaseSnafu {
                operation: "inspect expiry",
            })?;
        statement
            .query_map([now], |row| row.get::<_, i64>(0))
            .context(DatabaseSnafu {
                operation: "inspect expiry",
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context(DatabaseSnafu {
                operation: "inspect expiry",
            })?
            .into_iter()
            .map(|raw| OutboxId::from_stored(raw).ok_or(Error::InvalidId { value: raw }))
            .collect::<Result<Vec<_>>>()?
    };
    connection
        .execute(
            "UPDATE outbox SET state = 'expired', available_at = NULL WHERE state IN ('ready', \
             'deferred') AND expires_at <= ?1",
            [now],
        )
        .context(DatabaseSnafu {
            operation: "expire",
        })?;
    Ok(ids)
}
