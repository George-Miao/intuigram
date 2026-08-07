use std::fs::OpenOptions;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::thread::{self, JoinHandle};
use std::{fmt, fs};

use rusqlite::backup::Backup;
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use snafu::{ResultExt, Snafu};

use crate::{AccountId, StoreLayout};

mod cache_read;
mod database;
mod filesystem;
mod message_write;
mod migration;
mod model;
mod security;
mod selection;
mod session;
mod sync_write;
mod worker;

use cache_read::load_cache;
pub use database::AccountDatabase;
use filesystem::{prepare_data_directory, promote_without_replace, protect_path, run_worker};
use message_write::{
    delete_messages, save_chat_history, save_draft, save_messages, upsert_message,
};
pub(crate) use migration::open_and_migrate;
use migration::read_account_id;
pub use model::{
    CachedAccount, SessionMaterial, StoredChat, StoredDraft, StoredFolder, StoredMessage,
    StoredMutation, StoredSelection, SyncBatch, SyncCursor,
};
pub use security::{
    AccountCipher, Error as SecurityError, Result as SecurityResult, enable_local_lock,
    local_lock_is_enabled,
};
use selection::save_selection;
use session::read_session;
use sync_write::{apply_sync_mutation, commit_sync};
use worker::Command;
pub use worker::{AccountStore, DatabaseRequest};

mod migrations {
    refinery::embed_migrations!("migrations/account");
}

/// Failure while accessing an account database.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    /// The durable data directory could not be created.
    #[snafu(display("failed to create data directory {}", path.display()))]
    CreateDataDirectory {
        /// Directory that could not be created.
        path: PathBuf,
        /// Underlying filesystem failure.
        source: std::io::Error,
    },

    /// Owner-only permissions could not be applied.
    #[snafu(display("failed to protect data path {}", path.display()))]
    ProtectDataPath {
        /// Path whose permissions could not be changed.
        path: PathBuf,
        /// Underlying filesystem failure.
        source: std::io::Error,
    },

    /// The database engine could not open the account database.
    #[snafu(display("failed to open account database {}", path.display()))]
    OpenDatabase {
        /// Database path that could not be opened.
        path: PathBuf,
        /// Underlying database failure.
        source: rusqlite::Error,
    },

    /// A requested authorized Account database does not exist.
    #[snafu(display("account database does not exist at {}", path.display()))]
    MissingDatabase {
        /// Expected account database path.
        path: PathBuf,
    },

    /// A database worker thread could not be started.
    #[snafu(display("failed to start account database worker"))]
    SpawnWorker {
        /// Underlying thread creation failure.
        source: std::io::Error,
    },

    /// Embedded migrations could not be applied.
    #[snafu(display("failed to migrate account database {}", path.display()))]
    MigrateDatabase {
        /// Database path that could not be migrated.
        path: PathBuf,
        /// Underlying migration failure.
        source: refinery::Error,
    },

    /// The installed migration state could not be inspected.
    #[snafu(display("failed to inspect migrations in account database {}", path.display()))]
    InspectMigrations {
        /// Database path being inspected.
        path: PathBuf,
        /// Underlying database failure.
        source: rusqlite::Error,
    },

    /// A collision-safe pre-migration backup path could not be reserved.
    #[snafu(display(
        "failed to back up account database {} to {}",
        path.display(),
        backup.display()
    ))]
    ReserveBackup {
        /// Database being protected.
        path: PathBuf,
        /// Backup destination.
        backup: PathBuf,
        /// Underlying filesystem failure.
        source: std::io::Error,
    },

    /// The database engine could not snapshot a database before migration.
    #[snafu(display(
        "failed to snapshot account database {} to {}",
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

    /// No collision-safe pre-migration backup filename was available.
    #[snafu(display("could not reserve a backup filename for {}", path.display()))]
    BackupNamesExhausted {
        /// Database being protected.
        path: PathBuf,
    },

    /// A post-migration database check could not run.
    #[snafu(display("account database check could not run for {}: {check}", path.display()))]
    RunDatabaseCheck {
        /// Database that failed validation.
        path: PathBuf,
        /// Check that could not run.
        check: &'static str,
        /// Underlying database failure.
        source: rusqlite::Error,
    },

    /// A database failed a completed post-migration check.
    #[snafu(display("account database check failed for {}: {check}", path.display()))]
    DatabaseCheckFailed {
        /// Database that failed validation.
        path: PathBuf,
        /// Check that reported a failure.
        check: &'static str,
    },

    /// The stored account identity could not be read.
    #[snafu(display("failed to read the account identity"))]
    ReadIdentity {
        /// Underlying database failure.
        source: rusqlite::Error,
    },

    /// The authorized account identity could not be stored.
    #[snafu(display("failed to persist Telegram user ID {}", account.get()))]
    WriteIdentity {
        /// Telegram user ID being stored.
        account: AccountId,
        /// Underlying database failure.
        source: rusqlite::Error,
    },

    /// The pending database could not be renamed to its account path.
    #[snafu(display(
        "failed to promote pending database {} to {}",
        from.display(),
        to.display()
    ))]
    PromoteDatabase {
        /// Pending database path.
        from: PathBuf,
        /// Authorized account database path.
        to: PathBuf,
        /// Underlying filesystem failure.
        source: std::io::Error,
    },

    /// Promotion would overwrite an existing account database.
    #[snafu(display("account database already exists at {}", path.display()))]
    AccountAlreadyExists {
        /// Existing account database path.
        path: PathBuf,
    },

    /// The database worker stopped before completing an operation.
    #[snafu(display("account database worker is unavailable"))]
    WorkerUnavailable,

    /// The bounded database worker queue is full.
    #[snafu(display("account database worker queue is full"))]
    WorkerQueueFull,

    /// The database worker panicked while shutting down.
    #[snafu(display("account database worker panicked"))]
    WorkerPanicked,

    /// The database filename and persisted Telegram user ID disagree.
    #[snafu(display(
        "account database for {} contains identity {:?}",
        expected.get(),
        actual.map(AccountId::get)
    ))]
    IdentityMismatch {
        /// Telegram user ID implied by the filename.
        expected: AccountId,
        /// Telegram user ID stored inside the database.
        actual: Option<AccountId>,
    },

    /// The database contained a Telegram user ID outside the accepted domain.
    #[snafu(display("account database contains invalid Telegram user ID {value}"))]
    InvalidIdentity {
        /// Invalid stored value.
        value: i64,
    },

    /// The current `MTProto` session could not be read.
    #[snafu(display("failed to read the MTProto session"))]
    ReadSession {
        /// Underlying database failure.
        source: rusqlite::Error,
    },

    /// The current `MTProto` session could not be written.
    #[snafu(display("failed to persist the MTProto session"))]
    WriteSession {
        /// Underlying database failure.
        source: rusqlite::Error,
    },

    /// Stored authorization material did not contain exactly 256 bytes.
    #[snafu(display("stored MTProto authorization key has invalid length {length}"))]
    InvalidAuthorizationKey {
        /// Invalid number of bytes read from storage.
        length: usize,
    },

    /// A synchronized cache transaction could not be committed.
    #[snafu(display("failed to atomically persist synchronized Telegram records and cursor"))]
    CommitSync {
        /// Underlying database failure.
        source: rusqlite::Error,
    },

    /// Synchronized records could not be loaded.
    #[snafu(display("failed to load the synchronized Telegram cache"))]
    LoadCache {
        /// Underlying database failure.
        source: rusqlite::Error,
    },

    /// A Draft and its local recovery history could not be committed.
    #[snafu(display("failed to atomically persist the Draft for Chat {chat_id}"))]
    SaveDraft {
        /// Chat whose Draft could not be saved.
        chat_id: i64,

        /// Underlying database failure.
        source: rusqlite::Error,
    },

    /// Normalized history or local delivery state could not be persisted.
    #[snafu(display("failed to persist normalized Message records"))]
    SaveMessages {
        /// Underlying database failure.
        source: rusqlite::Error,
    },

    /// The last selected Folder and Chat could not be persisted.
    #[snafu(display("failed to persist the active Folder and Chat"))]
    SaveSelection {
        /// Underlying database failure.
        source: rusqlite::Error,
    },

    /// This build cannot enforce owner-only permissions on the platform.
    #[snafu(display("owner-only database permissions are unsupported on this platform"))]
    UnsupportedPermissions,
}

/// Result returned by account database operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[cfg(test)]
mod tests;
