use rusqlite::Connection;
use snafu::ResultExt;

use super::repository::{DatabaseSnafu, Result};

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
            "UPDATE outbox SET state = 'outcome_unknown', available_at = NULL, last_error = \
             'interrupted with remote outcome unknown' WHERE state = 'in_flight' AND operation = \
             'mutation'",
            [],
        )
        .context(DatabaseSnafu {
            operation: "recover",
        })?;
    transaction
        .execute(
            "UPDATE outbox SET state = 'outcome_unknown', available_at = NULL, last_error = \
             'interrupted while cancellation was pending' WHERE state = 'cancel_requested'",
            [],
        )
        .context(DatabaseSnafu {
            operation: "recover",
        })?;
    transaction.commit().context(DatabaseSnafu {
        operation: "recover",
    })
}
