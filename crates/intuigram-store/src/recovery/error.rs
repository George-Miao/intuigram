use std::path::PathBuf;

use snafu::Snafu;

use crate::account;

/// Failure while proving or performing a non-destructive Account recovery.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum RecoveryError {
    /// Unique Local Records could not be read completely.
    #[snafu(display(
        "cannot prove that unique Local Records are readable in {}",
        path.display()
    ))]
    ReadUniqueRecords {
        /// Failed Account database.
        path: PathBuf,

        /// SQLite read failure.
        source: rusqlite::Error,
    },

    /// Stored authorization material had an invalid length.
    #[snafu(display("authorization key in {} has invalid length {length}", path.display()))]
    InvalidAuthorizationKey {
        /// Failed Account database.
        path: PathBuf,

        /// Invalid byte count.
        length: usize,
    },

    /// No collision-safe temporary rebuild path was available.
    #[snafu(display("could not reserve a rebuild path beside {}", path.display()))]
    RebuildNamesExhausted {
        /// Failed Account database.
        path: PathBuf,
    },

    /// A collision-safe rebuild workspace could not be reserved.
    #[snafu(display("failed to reserve a rebuild workspace beside {}", path.display()))]
    ReserveRebuildWorkspace {
        /// Failed Account database.
        path: PathBuf,

        /// Filesystem failure.
        source: std::io::Error,
    },

    /// A fresh current-schema database could not be created.
    #[snafu(display("failed to create rebuilt Account database {}", path.display()))]
    CreateRebuiltDatabase {
        /// Temporary rebuild path.
        path: PathBuf,

        /// Storage failure.
        source: Box<account::Error>,
    },

    /// Verified unique records could not be copied atomically.
    #[snafu(display("failed to copy unique Local Records into {}", path.display()))]
    CopyUniqueRecords {
        /// Temporary rebuild path.
        path: PathBuf,

        /// SQLite write failure.
        source: rusqlite::Error,
    },

    /// The original database could not be moved to its recovery backup.
    #[snafu(display(
        "failed to preserve original Account database {} at {}",
        path.display(),
        backup.display()
    ))]
    PreserveOriginal {
        /// Original Account path.
        path: PathBuf,

        /// Recovery backup path.
        backup: PathBuf,

        /// Filesystem failure.
        source: std::io::Error,
    },

    /// The rebuilt database could not be installed after preserving the
    /// original.
    #[snafu(display("failed to install rebuilt Account database at {}", path.display()))]
    InstallRebuiltDatabase {
        /// Destination Account path.
        path: PathBuf,

        /// Filesystem failure.
        source: std::io::Error,
    },

    /// The installed rebuilt database could not be reopened safely.
    #[snafu(display("rebuilt Account database failed validation at {}", path.display()))]
    OpenRebuiltDatabase {
        /// Destination Account path.
        path: PathBuf,

        /// Storage failure.
        source: Box<account::Error>,
    },
}

/// Result of a recovery inspection or operation.
pub type RecoveryResult<T, E = RecoveryError> = std::result::Result<T, E>;
