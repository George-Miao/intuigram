//! Global-database failures with operation-level context.

use std::io;
use std::path::PathBuf;

use snafu::Snafu;

use crate::AccountId;

/// Failure while accessing the global database.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(super)))]
pub enum Error {
    /// The durable data directory could not be created.
    #[snafu(display("failed to create global data directory {}", path.display()))]
    CreateDataDirectory {
        /// Directory that could not be created.
        path: PathBuf,

        /// Underlying filesystem failure.
        source: io::Error,
    },

    /// Owner-only permissions could not be applied.
    #[snafu(display("failed to protect global data path {}", path.display()))]
    ProtectDataPath {
        /// Path whose permissions could not be changed.
        path: PathBuf,

        /// Underlying filesystem failure.
        source: io::Error,
    },

    /// Owner-only permissions cannot be enforced on this platform.
    #[snafu(display("owner-only global database permissions are unsupported on this platform"))]
    UnsupportedPermissions,

    /// A global database worker thread could not be started.
    #[snafu(display("failed to start global database worker"))]
    SpawnWorker {
        /// Underlying thread creation failure.
        source: io::Error,
    },

    /// The global database could not be opened.
    #[snafu(display("failed to open global database {}", path.display()))]
    OpenDatabase {
        /// Database path that could not be opened.
        path: PathBuf,

        /// Underlying database failure.
        source: rusqlite::Error,
    },

    /// Embedded global migrations could not be applied.
    #[snafu(display("failed to migrate global database {}", path.display()))]
    MigrateDatabase {
        /// Database path that could not be migrated.
        path: PathBuf,

        /// Underlying migration failure.
        source: refinery::Error,
    },

    /// The installed global migration state could not be inspected.
    #[snafu(display("failed to inspect migrations in global database {}", path.display()))]
    InspectMigrations {
        /// Database path being inspected.
        path: PathBuf,

        /// Underlying database failure.
        source: rusqlite::Error,
    },

    /// A collision-safe global backup path could not be reserved.
    #[snafu(display(
        "failed to reserve global database backup {} for {}",
        backup.display(),
        path.display()
    ))]
    ReserveBackup {
        /// Database being protected.
        path: PathBuf,

        /// Backup destination.
        backup: PathBuf,

        /// Underlying filesystem failure.
        source: io::Error,
    },

    /// The global database could not be snapshotted before migration.
    #[snafu(display(
        "failed to snapshot global database {} to {}",
        path.display(),
        backup.display()
    ))]
    BackupDatabase {
        /// Database being protected.
        path: PathBuf,

        /// Backup destination.
        backup: PathBuf,

        /// Underlying database failure.
        source: rusqlite::Error,
    },

    /// No collision-safe global backup filename was available.
    #[snafu(display("could not reserve a backup filename for {}", path.display()))]
    BackupNamesExhausted {
        /// Database being protected.
        path: PathBuf,
    },

    /// A post-migration global database check could not run.
    #[snafu(display("global database check could not run for {}: {check}", path.display()))]
    RunDatabaseCheck {
        /// Database that failed validation.
        path: PathBuf,

        /// Check that could not run.
        check: &'static str,

        /// Underlying database failure.
        source: rusqlite::Error,
    },

    /// A completed global database check reported a failure.
    #[snafu(display("global database check failed for {}: {check}", path.display()))]
    DatabaseCheckFailed {
        /// Database that failed validation.
        path: PathBuf,

        /// Check that reported a failure.
        check: &'static str,
    },

    /// An Account could not be registered.
    #[snafu(display("failed to register Telegram Account {}", account.get()))]
    RegisterAccount {
        /// Account being registered.
        account: AccountId,

        /// Underlying database failure.
        source: rusqlite::Error,
    },

    /// Registered Accounts could not be loaded.
    #[snafu(display("failed to load registered Telegram Accounts"))]
    ListAccounts {
        /// Underlying database failure.
        source: rusqlite::Error,
    },

    /// The global database contained an invalid Telegram user ID.
    #[snafu(display("global database contains invalid Telegram user ID {value}"))]
    InvalidAccountId {
        /// Invalid stored value.
        value: i64,
    },

    /// The database worker stopped before completing an operation.
    #[snafu(display("global database worker is unavailable"))]
    WorkerUnavailable,

    /// The database worker panicked while shutting down.
    #[snafu(display("global database worker panicked"))]
    WorkerPanicked,
}

/// Result returned by global database operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;
