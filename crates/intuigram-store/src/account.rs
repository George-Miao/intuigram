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

    /// This build cannot enforce owner-only permissions on the platform.
    #[snafu(display("owner-only database permissions are unsupported on this platform"))]
    UnsupportedPermissions,
}

/// Result returned by account database operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Durable connection material for one Telegram data-center authorization.
#[derive(Clone, Eq, PartialEq)]
pub struct SessionMaterial {
    /// Telegram data-center number.
    pub dc_id: i32,
    /// Direct TCP endpoint associated with this authorization.
    pub endpoint: String,
    /// Secret authorization key. Never include this value in diagnostics.
    auth_key: [u8; 256],
    /// Difference between local and Telegram server time.
    pub time_offset: i32,
    /// Most recently known server salt.
    pub first_salt: i64,
}

impl fmt::Debug for SessionMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SessionMaterial")
            .field("dc_id", &self.dc_id)
            .field("endpoint", &self.endpoint)
            .field("auth_key", &"[REDACTED]")
            .field("time_offset", &self.time_offset)
            .field("first_salt", &self.first_salt)
            .finish()
    }
}

impl SessionMaterial {
    /// Creates durable session material.
    #[must_use]
    pub const fn new(
        dc_id: i32,
        endpoint: String,
        auth_key: [u8; 256],
        time_offset: i32,
        first_salt: i64,
    ) -> Self {
        Self {
            dc_id,
            endpoint,
            auth_key,
            time_offset,
            first_salt,
        }
    }

    /// Copies the secret key into the protocol adapter.
    #[must_use]
    pub const fn auth_key(&self) -> [u8; 256] {
        self.auth_key
    }
}

/// Telegram synchronization cursor committed with normalized records.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SyncCursor {
    /// Logical synchronization scope, normally `account` or a Channel ID.
    pub scope: String,

    /// Telegram persistent timestamp.
    pub pts: i32,

    /// Telegram secret-chat timestamp retained for protocol completeness.
    pub qts: i32,

    /// Telegram server date.
    pub date: i32,

    /// Telegram global update sequence.
    pub seq: i32,
}

/// Store-owned normalized Folder record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredFolder {
    /// Telegram Folder ID.
    pub id: i32,

    /// Display title.
    pub title: String,

    /// Aggregate unread count.
    pub unread: u32,
}

/// Store-owned normalized Chat record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredChat {
    /// Stable marked Telegram peer ID.
    pub id: i64,

    /// Stable textual normalized Chat kind.
    pub kind: String,

    /// Display title.
    pub title: String,

    /// Last-message fallback.
    pub preview: String,

    /// Unread count.
    pub unread: u32,

    /// Telegram pin state.
    pub pinned: bool,

    /// Folder IDs in which the Chat appears.
    pub folders: Vec<i32>,
}

/// Store-owned normalized Message record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredMessage {
    /// Owning Chat.
    pub chat_id: i64,

    /// Telegram or pending local Message ID.
    pub id: i64,

    /// Sender display fallback.
    pub sender: String,

    /// Searchable semantic text fallback.
    pub body: String,

    /// Compact presentation timestamp.
    pub timestamp: String,

    /// Stable textual direction.
    pub direction: String,

    /// Stable textual delivery state.
    pub delivery: String,

    /// Replied-to Message, when any.
    pub reply_to: Option<i64>,

    /// Thread root, or `None` for root Chat history.
    pub thread_root: Option<i64>,

    /// Stable semantic content kind.
    pub content_kind: String,

    /// Forward-compatible normalized metadata.
    pub metadata: String,
}

/// One atomic synchronized-cache commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncBatch {
    /// Cursor advanced by this exact record set.
    pub cursor: SyncCursor,

    /// Folder records to upsert in server order.
    pub folders: Vec<StoredFolder>,

    /// Chat records to upsert.
    pub chats: Vec<StoredChat>,

    /// Message records to upsert.
    pub messages: Vec<StoredMessage>,
}

/// Durable Draft value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredDraft {
    /// Owning Chat.
    pub chat_id: i64,

    /// Thread root, or `None` for the root Chat Draft.
    pub thread_root: Option<i64>,

    /// Draft text.
    pub text: String,

    /// Replied-to Message, when any.
    pub reply_to: Option<i64>,

    /// Unix timestamp used for last-writer conflict resolution.
    pub modified_at: i64,
}

/// Immediately renderable durable Account cache.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CachedAccount {
    /// Last durable synchronization cursors.
    pub cursors: Vec<SyncCursor>,

    /// Folders in display order.
    pub folders: Vec<StoredFolder>,

    /// Cached Chats.
    pub chats: Vec<StoredChat>,

    /// Cached Messages.
    pub messages: Vec<StoredMessage>,

    /// Current durable Drafts.
    pub drafts: Vec<StoredDraft>,
}

enum Command {
    ReadIdentity {
        reply: SyncSender<Result<Option<AccountId>>>,
    },
    WriteIdentity {
        account: AccountId,
        reply: SyncSender<Result<()>>,
    },
    ReadSession {
        reply: SyncSender<Result<Option<SessionMaterial>>>,
    },
    WriteSession {
        session: Box<SessionMaterial>,
        reply: SyncSender<Result<()>>,
    },
    CommitSync {
        batch: Box<SyncBatch>,
        reply: SyncSender<Result<()>>,
    },
    LoadCache {
        reply: SyncSender<Result<CachedAccount>>,
    },
    SaveDraft {
        draft: StoredDraft,
        reply: SyncSender<Result<()>>,
    },
    CommitSyncAsync {
        batch: Box<SyncBatch>,
        reply: AsyncReply<()>,
    },
    SaveDraftAsync {
        draft: StoredDraft,
        reply: AsyncReply<()>,
    },
    SaveMessagesAsync {
        messages: Vec<StoredMessage>,
        reply: AsyncReply<()>,
    },
    Shutdown,
}

struct AsyncState<T> {
    result: Option<Result<T>>,
    waker: Option<Waker>,
}

struct AsyncReply<T> {
    state: Arc<Mutex<AsyncState<T>>>,
}

impl<T> AsyncReply<T> {
    fn finish(self, result: Result<T>) {
        let mut state = self
            .state
            .lock()
            .expect("database response mutex is not exposed to panicking user callbacks");
        state.result = Some(result);
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }
}

/// Awaitable response from the dedicated blocking SQLite worker.
pub struct DatabaseRequest<T> {
    state: Arc<Mutex<AsyncState<T>>>,
}

impl<T> Future for DatabaseRequest<T> {
    type Output = Result<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self
            .state
            .lock()
            .expect("database response mutex is not exposed to panicking user callbacks");
        if let Some(result) = state.result.take() {
            Poll::Ready(result)
        } else {
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

fn async_response<T>() -> (AsyncReply<T>, DatabaseRequest<T>) {
    let state = Arc::new(Mutex::new(AsyncState {
        result: None,
        waker: None,
    }));
    (
        AsyncReply {
            state: Arc::clone(&state),
        },
        DatabaseRequest { state },
    )
}

/// Cloneable nonblocking request endpoint for an Account database worker.
#[derive(Clone)]
pub struct AccountStore {
    commands: SyncSender<Command>,
}

impl AccountStore {
    /// Enqueues an atomic synchronized-cache commit without blocking the
    /// runtime.
    pub fn commit_sync(&self, batch: SyncBatch) -> Result<DatabaseRequest<()>> {
        let (reply, request) = async_response();
        self.commands
            .try_send(Command::CommitSyncAsync {
                batch: Box::new(batch),
                reply,
            })
            .map_err(map_try_send_error)?;
        Ok(request)
    }

    /// Enqueues a durable Draft commit without blocking the runtime.
    pub fn save_draft(&self, draft: StoredDraft) -> Result<DatabaseRequest<()>> {
        let (reply, request) = async_response();
        self.commands
            .try_send(Command::SaveDraftAsync { draft, reply })
            .map_err(map_try_send_error)?;
        Ok(request)
    }

    /// Enqueues an atomic normalized Message upsert without advancing a cursor.
    pub fn save_messages(&self, messages: Vec<StoredMessage>) -> Result<DatabaseRequest<()>> {
        let (reply, request) = async_response();
        self.commands
            .try_send(Command::SaveMessagesAsync { messages, reply })
            .map_err(map_try_send_error)?;
        Ok(request)
    }
}

fn map_try_send_error<T>(error: mpsc::TrySendError<T>) -> Error {
    match error {
        mpsc::TrySendError::Full(_) => Error::WorkerQueueFull,
        mpsc::TrySendError::Disconnected(_) => Error::WorkerUnavailable,
    }
}

/// A database containing one Telegram account's durable state.
pub struct AccountDatabase {
    commands: SyncSender<Command>,
    worker: Option<JoinHandle<()>>,
}

impl AccountDatabase {
    /// Returns a cloneable nonblocking endpoint for runtime adapter tasks.
    #[must_use]
    pub fn store(&self) -> AccountStore {
        AccountStore {
            commands: self.commands.clone(),
        }
    }

    /// Creates and migrates the database used during login.
    pub fn begin_login(layout: &StoreLayout) -> Result<Self> {
        Self::spawn(layout.pending_database(), true)
    }

    /// Stores the authorized Telegram user ID and atomically promotes the
    /// database.
    pub fn finish_login(mut self, layout: &StoreLayout, account: AccountId) -> Result<Self> {
        let target = layout.account_database(account);
        self.write_account_id(account)?;
        self.stop()?;
        let pending = layout.pending_database();
        if let Err(source) = promote_without_replace(&pending, &target) {
            if source.kind() == std::io::ErrorKind::AlreadyExists {
                return AccountAlreadyExistsSnafu { path: target }.fail();
            }
            return Err(Error::PromoteDatabase {
                from: pending,
                to: target,
                source,
            });
        }
        Self::spawn(target, false)
    }

    /// Opens a previously authorized account database.
    pub fn open(layout: &StoreLayout, account: AccountId) -> Result<Self> {
        let path = layout.account_database(account);
        if !path.is_file() {
            return MissingDatabaseSnafu { path }.fail();
        }
        let database = Self::spawn(path, false)?;
        let actual = database.account_id()?;
        if actual != Some(account) {
            return IdentityMismatchSnafu {
                expected: account,
                actual,
            }
            .fail();
        }
        Ok(database)
    }

    /// Returns the Telegram user ID persisted in this database.
    pub fn account_id(&self) -> Result<Option<AccountId>> {
        let (reply, response) = mpsc::sync_channel(1);
        self.commands
            .send(Command::ReadIdentity { reply })
            .map_err(|_| Error::WorkerUnavailable)?;
        response.recv().map_err(|_| Error::WorkerUnavailable)?
    }

    /// Returns the current durable Telegram authorization, when present.
    pub fn session(&self) -> Result<Option<SessionMaterial>> {
        let (reply, response) = mpsc::sync_channel(1);
        self.commands
            .send(Command::ReadSession { reply })
            .map_err(|_| Error::WorkerUnavailable)?;
        response.recv().map_err(|_| Error::WorkerUnavailable)?
    }

    /// Persists a Telegram authorization before it can be used by the UI.
    pub fn save_session(&self, session: SessionMaterial) -> Result<()> {
        let (reply, response) = mpsc::sync_channel(1);
        self.commands
            .send(Command::WriteSession {
                session: Box::new(session),
                reply,
            })
            .map_err(|_| Error::WorkerUnavailable)?;
        response.recv().map_err(|_| Error::WorkerUnavailable)?
    }

    /// Atomically upserts normalized records and advances their cursor.
    pub fn commit_sync(&self, batch: SyncBatch) -> Result<()> {
        let (reply, response) = mpsc::sync_channel(1);
        self.commands
            .send(Command::CommitSync {
                batch: Box::new(batch),
                reply,
            })
            .map_err(|_| Error::WorkerUnavailable)?;
        response.recv().map_err(|_| Error::WorkerUnavailable)?
    }

    /// Loads the complete immediately renderable synchronized cache.
    pub fn cached_account(&self) -> Result<CachedAccount> {
        let (reply, response) = mpsc::sync_channel(1);
        self.commands
            .send(Command::LoadCache { reply })
            .map_err(|_| Error::WorkerUnavailable)?;
        response.recv().map_err(|_| Error::WorkerUnavailable)?
    }

    /// Persists a Draft before callers report it as saved.
    pub fn save_draft(&self, draft: StoredDraft) -> Result<()> {
        let (reply, response) = mpsc::sync_channel(1);
        self.commands
            .send(Command::SaveDraft { draft, reply })
            .map_err(|_| Error::WorkerUnavailable)?;
        response.recv().map_err(|_| Error::WorkerUnavailable)?
    }

    fn spawn(path: PathBuf, create: bool) -> Result<Self> {
        prepare_data_directory(&path)?;
        let (commands, requests) = mpsc::sync_channel(32);
        let (ready, initialized) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("intuigram-account-db".to_owned())
            .spawn(move || run_worker(&path, create, &requests, &ready))
            .context(SpawnWorkerSnafu)?;
        initialized.recv().map_err(|_| Error::WorkerUnavailable)??;
        Ok(Self {
            commands,
            worker: Some(worker),
        })
    }

    fn write_account_id(&self, account: AccountId) -> Result<()> {
        let (reply, response) = mpsc::sync_channel(1);
        self.commands
            .send(Command::WriteIdentity { account, reply })
            .map_err(|_| Error::WorkerUnavailable)?;
        response.recv().map_err(|_| Error::WorkerUnavailable)?
    }

    fn stop(&mut self) -> Result<()> {
        self.commands
            .send(Command::Shutdown)
            .map_err(|_| Error::WorkerUnavailable)?;
        self.worker
            .take()
            .ok_or(Error::WorkerUnavailable)?
            .join()
            .map_err(|_| Error::WorkerPanicked)
    }
}

impl Drop for AccountDatabase {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn prepare_data_directory(database: &Path) -> Result<()> {
    let directory = database.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(directory).context(CreateDataDirectorySnafu {
        path: directory.to_path_buf(),
    })?;
    protect_path(directory, true)
}

#[cfg(unix)]
fn protect_path(path: &Path, directory: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mode = if directory { 0o700 } else { 0o600 };
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).context(ProtectDataPathSnafu {
        path: path.to_path_buf(),
    })
}

#[cfg(not(unix))]
fn protect_path(_path: &Path, _directory: bool) -> Result<()> {
    UnsupportedPermissionsSnafu.fail()
}

#[cfg(any(unix, target_os = "wasi"))]
fn promote_without_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};

    renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE).map_err(std::io::Error::from)
}

#[cfg(not(any(unix, target_os = "wasi")))]
fn promote_without_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::rename(from, to)
}

fn run_worker(
    path: &Path,
    create: bool,
    requests: &Receiver<Command>,
    ready: &SyncSender<Result<()>>,
) {
    let connection = open_and_migrate(path, create);
    let Ok(connection) = connection else {
        let _ = ready.send(connection.map(|_| ()));
        return;
    };
    if ready.send(Ok(())).is_err() {
        return;
    }

    while let Ok(command) = requests.recv() {
        match command {
            Command::ReadIdentity { reply } => {
                let _ = reply.send(read_account_id(&connection));
            }
            Command::WriteIdentity { account, reply } => {
                let result = connection
                    .execute(
                        "INSERT OR REPLACE INTO account_identity (singleton, telegram_user_id) \
                         VALUES (1, ?1)",
                        params![account.get()],
                    )
                    .map(|_| ())
                    .context(WriteIdentitySnafu { account });
                let _ = reply.send(result);
            }
            Command::ReadSession { reply } => {
                let _ = reply.send(read_session(&connection));
            }
            Command::WriteSession { session, reply } => {
                let result = connection
                    .execute(
                        "INSERT OR REPLACE INTO mtproto_session (singleton, dc_id, endpoint, \
                         auth_key, time_offset, first_salt) VALUES (1, ?1, ?2, ?3, ?4, ?5)",
                        params![
                            session.dc_id,
                            session.endpoint,
                            session.auth_key.as_slice(),
                            session.time_offset,
                            session.first_salt
                        ],
                    )
                    .map(|_| ())
                    .context(WriteSessionSnafu);
                let _ = reply.send(result);
            }
            Command::CommitSync { batch, reply } => {
                let _ = reply.send(commit_sync(&connection, *batch));
            }
            Command::LoadCache { reply } => {
                let _ = reply.send(load_cache(&connection));
            }
            Command::SaveDraft { draft, reply } => {
                let _ = reply.send(save_draft(&connection, draft));
            }
            Command::CommitSyncAsync { batch, reply } => {
                reply.finish(commit_sync(&connection, *batch));
            }
            Command::SaveDraftAsync { draft, reply } => {
                reply.finish(save_draft(&connection, draft));
            }
            Command::SaveMessagesAsync { messages, reply } => {
                reply.finish(save_messages(&connection, messages));
            }
            Command::Shutdown => break,
        }
    }
}

fn open_and_migrate(path: &Path, create: bool) -> Result<Connection> {
    let existed = path.is_file();
    let mut flags = OpenFlags::SQLITE_OPEN_READ_WRITE;
    if create {
        flags |= OpenFlags::SQLITE_OPEN_CREATE;
    }
    let mut connection = Connection::open_with_flags(path, flags).context(OpenDatabaseSnafu {
        path: path.to_path_buf(),
    })?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON")
        .context(OpenDatabaseSnafu {
            path: path.to_path_buf(),
        })?;
    protect_path(path, false)?;
    let runner = migrations::migrations::runner();
    if existed {
        let has_history: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = \
                 'refinery_schema_history')",
                [],
                |row| row.get(0),
            )
            .context(InspectMigrationsSnafu {
                path: path.to_path_buf(),
            })?;
        let needs_migration = if has_history {
            let applied =
                runner
                    .get_applied_migrations(&mut connection)
                    .context(MigrateDatabaseSnafu {
                        path: path.to_path_buf(),
                    })?;
            runner
                .get_migrations()
                .iter()
                .any(|migration| !applied.contains(migration))
        } else {
            !runner.get_migrations().is_empty()
        };
        if needs_migration {
            create_backup(&connection, path)?;
        }
    }
    runner.run(&mut connection).context(MigrateDatabaseSnafu {
        path: path.to_path_buf(),
    })?;
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .context(RunDatabaseCheckSnafu {
            path: path.to_path_buf(),
            check: "integrity_check",
        })?;
    if integrity != "ok" {
        return DatabaseCheckFailedSnafu {
            path: path.to_path_buf(),
            check: "integrity_check",
        }
        .fail();
    }
    let foreign_key_failure = connection
        .prepare("PRAGMA foreign_key_check")
        .and_then(|mut statement| statement.exists([]))
        .context(RunDatabaseCheckSnafu {
            path: path.to_path_buf(),
            check: "foreign_key_check",
        })?;
    if foreign_key_failure {
        return DatabaseCheckFailedSnafu {
            path: path.to_path_buf(),
            check: "foreign_key_check",
        }
        .fail();
    }
    Ok(connection)
}

fn create_backup(source: &Connection, path: &Path) -> Result<PathBuf> {
    for attempt in 1..=1_000_u16 {
        let backup = path.with_extension(format!("db.pre-migration-{attempt}.bak"));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&backup)
        {
            Ok(file) => drop(file),
            Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(Error::ReserveBackup {
                    path: path.to_path_buf(),
                    backup,
                    source,
                });
            }
        }
        let mut destination =
            Connection::open_with_flags(&backup, OpenFlags::SQLITE_OPEN_READ_WRITE).context(
                BackupDatabaseSnafu {
                    path: path.to_path_buf(),
                    backup: backup.clone(),
                },
            )?;
        Backup::new(source, &mut destination)
            .and_then(|snapshot| {
                snapshot.run_to_completion(128, std::time::Duration::from_millis(10), None)
            })
            .context(BackupDatabaseSnafu {
                path: path.to_path_buf(),
                backup: backup.clone(),
            })?;
        drop(destination);
        protect_path(&backup, false)?;
        return Ok(backup);
    }
    BackupNamesExhaustedSnafu {
        path: path.to_path_buf(),
    }
    .fail()
}

fn read_account_id(connection: &Connection) -> Result<Option<AccountId>> {
    let value = connection
        .query_row(
            "SELECT telegram_user_id FROM account_identity WHERE singleton = 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .context(ReadIdentitySnafu)?;
    value
        .map(|raw| AccountId::new(raw).ok_or(Error::InvalidIdentity { value: raw }))
        .transpose()
}

fn read_session(connection: &Connection) -> Result<Option<SessionMaterial>> {
    let row = connection
        .query_row(
            "SELECT dc_id, endpoint, auth_key, time_offset, first_salt FROM mtproto_session WHERE \
             singleton = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i32>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, i32>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()
        .context(ReadSessionSnafu)?;
    row.map(|(dc_id, endpoint, key, time_offset, first_salt)| {
        let length = key.len();
        let auth_key = key
            .try_into()
            .map_err(|_| Error::InvalidAuthorizationKey { length })?;
        Ok(SessionMaterial::new(
            dc_id,
            endpoint,
            auth_key,
            time_offset,
            first_salt,
        ))
    })
    .transpose()
}

fn commit_sync(connection: &Connection, batch: SyncBatch) -> Result<()> {
    let transaction = connection
        .unchecked_transaction()
        .context(CommitSyncSnafu)?;
    transaction
        .execute(
            "INSERT INTO sync_state(scope, pts, qts, date, seq) VALUES (?1, ?2, ?3, ?4, ?5) ON \
             CONFLICT(scope) DO UPDATE SET pts=excluded.pts, qts=excluded.qts, \
             date=excluded.date, seq=excluded.seq",
            params![
                batch.cursor.scope,
                batch.cursor.pts,
                batch.cursor.qts,
                batch.cursor.date,
                batch.cursor.seq
            ],
        )
        .context(CommitSyncSnafu)?;
    if !batch.folders.is_empty() {
        transaction
            .execute("DELETE FROM folders", [])
            .context(CommitSyncSnafu)?;
        for (position, folder) in batch.folders.into_iter().enumerate() {
            let position = i64::try_from(position)
                .expect("an in-memory Folder list cannot exceed SQLite's signed index range");
            transaction
                .execute(
                    "INSERT INTO folders(folder_id, title, unread_count, position) VALUES (?1, \
                     ?2, ?3, ?4)",
                    params![folder.id, folder.title, folder.unread, position],
                )
                .context(CommitSyncSnafu)?;
        }
    }
    for chat in batch.chats {
        transaction
            .execute(
                "INSERT INTO chats(chat_id, kind, title, preview, unread_count, pinned) VALUES \
                 (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(chat_id) DO UPDATE SET kind=excluded.kind, \
                 title=excluded.title, preview=excluded.preview, \
                 unread_count=excluded.unread_count, pinned=excluded.pinned",
                params![
                    chat.id,
                    chat.kind,
                    chat.title,
                    chat.preview,
                    chat.unread,
                    chat.pinned
                ],
            )
            .context(CommitSyncSnafu)?;
        transaction
            .execute("DELETE FROM chat_folders WHERE chat_id = ?1", [chat.id])
            .context(CommitSyncSnafu)?;
        for (position, folder) in chat.folders.into_iter().enumerate() {
            let position = i64::try_from(position)
                .expect("an in-memory Folder list cannot exceed SQLite's signed index range");
            transaction
                .execute(
                    "INSERT INTO chat_folders(chat_id, folder_id, position) VALUES (?1, ?2, ?3)",
                    params![chat.id, folder, position],
                )
                .context(CommitSyncSnafu)?;
        }
    }
    for message in batch.messages {
        upsert_message(&transaction, &message).context(CommitSyncSnafu)?;
    }
    transaction.commit().context(CommitSyncSnafu)
}

fn save_messages(connection: &Connection, messages: Vec<StoredMessage>) -> Result<()> {
    let transaction = connection
        .unchecked_transaction()
        .context(SaveMessagesSnafu)?;
    for message in messages {
        upsert_message(&transaction, &message).context(SaveMessagesSnafu)?;
    }
    transaction.commit().context(SaveMessagesSnafu)
}

fn upsert_message(connection: &Connection, message: &StoredMessage) -> rusqlite::Result<()> {
    connection
        .execute(
            "INSERT INTO messages(chat_id, message_id, sender, body, timestamp, direction, \
             delivery, reply_to_message_id, thread_root_message_id, content_kind, metadata) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) ON CONFLICT(chat_id, \
             message_id) DO UPDATE SET sender=excluded.sender, body=excluded.body, \
             timestamp=excluded.timestamp, direction=excluded.direction, \
             delivery=excluded.delivery, reply_to_message_id=excluded.reply_to_message_id, \
             thread_root_message_id=excluded.thread_root_message_id, \
             content_kind=excluded.content_kind, metadata=excluded.metadata",
            params![
                message.chat_id,
                message.id,
                message.sender,
                message.body,
                message.timestamp,
                message.direction,
                message.delivery,
                message.reply_to,
                message.thread_root,
                message.content_kind,
                message.metadata
            ],
        )
        .map(|_| ())
}

fn save_draft(connection: &Connection, draft: StoredDraft) -> Result<()> {
    let thread_root = draft.thread_root.unwrap_or(0);
    let transaction = connection.unchecked_transaction().context(SaveDraftSnafu {
        chat_id: draft.chat_id,
    })?;
    let prior = transaction
        .query_row(
            "SELECT text, reply_to_message_id, modified_at FROM drafts WHERE chat_id = ?1 AND \
             thread_root_message_id = ?2",
            params![draft.chat_id, thread_root],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()
        .context(SaveDraftSnafu {
            chat_id: draft.chat_id,
        })?;
    if let Some((text, reply_to, modified_at)) = prior
        && (text != draft.text || reply_to != draft.reply_to)
    {
        transaction
            .execute(
                "INSERT INTO draft_history(chat_id, thread_root_message_id, text, \
                 reply_to_message_id, displaced_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![draft.chat_id, thread_root, text, reply_to, modified_at],
            )
            .context(SaveDraftSnafu {
                chat_id: draft.chat_id,
            })?;
    }
    transaction
        .execute(
            "INSERT INTO drafts(chat_id, thread_root_message_id, text, reply_to_message_id, \
             modified_at) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(chat_id, \
             thread_root_message_id) DO UPDATE SET text=excluded.text, \
             reply_to_message_id=excluded.reply_to_message_id, modified_at=excluded.modified_at",
            params![
                draft.chat_id,
                thread_root,
                draft.text,
                draft.reply_to,
                draft.modified_at
            ],
        )
        .context(SaveDraftSnafu {
            chat_id: draft.chat_id,
        })?;
    transaction
        .execute(
            "DELETE FROM draft_history WHERE chat_id = ?1 AND thread_root_message_id = ?2 AND \
             version_id NOT IN (SELECT version_id FROM draft_history WHERE chat_id = ?1 AND \
             thread_root_message_id = ?2 ORDER BY displaced_at DESC, version_id DESC LIMIT 20)",
            params![draft.chat_id, thread_root],
        )
        .context(SaveDraftSnafu {
            chat_id: draft.chat_id,
        })?;
    transaction.commit().context(SaveDraftSnafu {
        chat_id: draft.chat_id,
    })
}

fn load_cache(connection: &Connection) -> Result<CachedAccount> {
    let cursors = load_cursors(connection)?;
    let folders = load_folders(connection)?;
    let mut chats = load_chats(connection)?;
    for chat in &mut chats {
        let mut statement = connection
            .prepare("SELECT folder_id FROM chat_folders WHERE chat_id = ?1 ORDER BY position")
            .context(LoadCacheSnafu)?;
        chat.folders = statement
            .query_map([chat.id], |row| row.get(0))
            .context(LoadCacheSnafu)?
            .collect::<std::result::Result<_, _>>()
            .context(LoadCacheSnafu)?;
    }
    let messages = load_messages(connection)?;
    let drafts = load_drafts(connection)?;
    Ok(CachedAccount {
        cursors,
        folders,
        chats,
        messages,
        drafts,
    })
}

fn load_cursors(connection: &Connection) -> Result<Vec<SyncCursor>> {
    let mut statement = connection
        .prepare("SELECT scope, pts, qts, date, seq FROM sync_state ORDER BY scope")
        .context(LoadCacheSnafu)?;
    statement
        .query_map([], |row| {
            Ok(SyncCursor {
                scope: row.get(0)?,
                pts: row.get(1)?,
                qts: row.get(2)?,
                date: row.get(3)?,
                seq: row.get(4)?,
            })
        })
        .context(LoadCacheSnafu)?
        .collect::<std::result::Result<_, _>>()
        .context(LoadCacheSnafu)
}

fn load_folders(connection: &Connection) -> Result<Vec<StoredFolder>> {
    let mut statement = connection
        .prepare("SELECT folder_id, title, unread_count FROM folders ORDER BY position")
        .context(LoadCacheSnafu)?;
    statement
        .query_map([], |row| {
            let unread = row.get::<_, i64>(2)?;
            Ok(StoredFolder {
                id: row.get(0)?,
                title: row.get(1)?,
                unread: u32::try_from(unread).unwrap_or(0),
            })
        })
        .context(LoadCacheSnafu)?
        .collect::<std::result::Result<_, _>>()
        .context(LoadCacheSnafu)
}

fn load_chats(connection: &Connection) -> Result<Vec<StoredChat>> {
    let mut statement = connection
        .prepare(
            "SELECT chat_id, kind, title, preview, unread_count, pinned FROM chats ORDER BY \
             pinned DESC, chat_id DESC",
        )
        .context(LoadCacheSnafu)?;
    statement
        .query_map([], |row| {
            let unread = row.get::<_, i64>(4)?;
            Ok(StoredChat {
                id: row.get(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                preview: row.get(3)?,
                unread: u32::try_from(unread).unwrap_or(0),
                pinned: row.get(5)?,
                folders: Vec::new(),
            })
        })
        .context(LoadCacheSnafu)?
        .collect::<std::result::Result<_, _>>()
        .context(LoadCacheSnafu)
}

fn load_messages(connection: &Connection) -> Result<Vec<StoredMessage>> {
    let mut statement = connection
        .prepare(
            "SELECT chat_id, message_id, sender, body, timestamp, direction, delivery, \
             reply_to_message_id, thread_root_message_id, content_kind, metadata FROM messages \
             ORDER BY chat_id, message_id",
        )
        .context(LoadCacheSnafu)?;
    statement
        .query_map([], |row| {
            Ok(StoredMessage {
                chat_id: row.get(0)?,
                id: row.get(1)?,
                sender: row.get(2)?,
                body: row.get(3)?,
                timestamp: row.get(4)?,
                direction: row.get(5)?,
                delivery: row.get(6)?,
                reply_to: row.get(7)?,
                thread_root: row.get(8)?,
                content_kind: row.get(9)?,
                metadata: row.get(10)?,
            })
        })
        .context(LoadCacheSnafu)?
        .collect::<std::result::Result<_, _>>()
        .context(LoadCacheSnafu)
}

fn load_drafts(connection: &Connection) -> Result<Vec<StoredDraft>> {
    let mut statement = connection
        .prepare(
            "SELECT chat_id, thread_root_message_id, text, reply_to_message_id, modified_at FROM \
             drafts ORDER BY chat_id, thread_root_message_id",
        )
        .context(LoadCacheSnafu)?;
    statement
        .query_map([], |row| {
            Ok(StoredDraft {
                chat_id: row.get(0)?,
                thread_root: match row.get::<_, i64>(1)? {
                    0 => None,
                    root => Some(root),
                },
                text: row.get(2)?,
                reply_to: row.get(3)?,
                modified_at: row.get(4)?,
            })
        })
        .context(LoadCacheSnafu)?
        .collect::<std::result::Result<_, _>>()
        .context(LoadCacheSnafu)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rusqlite::Connection;
    use tempfile::tempdir;

    use super::{
        AccountDatabase, SessionMaterial, StoredChat, StoredDraft, StoredFolder, StoredMessage,
        SyncBatch, SyncCursor,
    };
    use crate::{AccountId, StoreLayout};

    #[test]
    fn pending_login_is_promoted_to_a_persistent_account_database() {
        let temporary = tempdir().expect("temporary directory should be created");
        let layout = StoreLayout::new(temporary.path().join("intuigram"));
        let account = AccountId::new(4_242).expect("fixture ID should be positive");

        let pending =
            AccountDatabase::begin_login(&layout).expect("pending login database should open");
        assert_eq!(pending.account_id().expect("identity should be read"), None);

        let authorized = pending
            .finish_login(&layout, account)
            .expect("pending database should be promoted");
        assert_eq!(
            authorized.account_id().expect("identity should be read"),
            Some(account)
        );
        drop(authorized);

        let reopened = AccountDatabase::open(&layout, account)
            .expect("promoted account database should reopen");
        assert_eq!(
            reopened.account_id().expect("identity should persist"),
            Some(account)
        );
        assert!(!layout.pending_database().exists());
        assert!(layout.account_database(account).exists());
    }

    #[test]
    fn mtproto_session_round_trips_without_appearing_in_debug_output() {
        let temporary = tempdir().expect("temporary directory should be created");
        let layout = StoreLayout::new(temporary.path().join("intuigram"));
        let database =
            AccountDatabase::begin_login(&layout).expect("pending login database should open");
        let session = SessionMaterial::new(2, "149.154.167.40:443".to_owned(), [0xa5; 256], -2, 42);

        database
            .save_session(session.clone())
            .expect("session should persist");

        assert_eq!(
            database.session().expect("session should load"),
            Some(session.clone())
        );
        assert!(!format!("{session:?}").contains("a5"));
        assert!(format!("{session:?}").contains("[REDACTED]"));
    }

    #[test]
    fn opening_a_missing_account_does_not_create_a_database() {
        let temporary = tempdir().expect("temporary directory should be created");
        let layout = StoreLayout::new(temporary.path().join("intuigram"));
        let account = AccountId::new(7).expect("fixture ID should be positive");

        assert!(AccountDatabase::open(&layout, account).is_err());
        assert!(!layout.account_database(account).exists());
    }

    #[test]
    fn promotion_never_replaces_an_existing_account_database() {
        let temporary = tempdir().expect("temporary directory should be created");
        let layout = StoreLayout::new(temporary.path().join("intuigram"));
        fs::create_dir_all(layout.data_directory()).expect("data directory should be created");
        let account = AccountId::new(8).expect("fixture ID should be positive");
        let target = layout.account_database(account);
        fs::write(&target, b"existing account")
            .expect("existing account fixture should be written");
        let pending =
            AccountDatabase::begin_login(&layout).expect("pending login database should open");

        assert!(pending.finish_login(&layout, account).is_err());
        assert_eq!(
            fs::read(target).expect("existing account fixture should remain"),
            b"existing account"
        );
        assert!(layout.pending_database().exists());
    }

    #[test]
    fn an_existing_unmigrated_database_is_backed_up_before_migration() {
        let temporary = tempdir().expect("temporary directory should be created");
        let layout = StoreLayout::new(temporary.path().join("intuigram"));
        fs::create_dir_all(layout.data_directory()).expect("data directory should be created");
        let pending_path = layout.pending_database();
        let connection = Connection::open(&pending_path).expect("fixture database should open");
        connection
            .execute("CREATE TABLE legacy(value TEXT NOT NULL)", [])
            .expect("legacy schema should be created");
        drop(connection);

        let database =
            AccountDatabase::begin_login(&layout).expect("legacy database should migrate safely");
        drop(database);

        let backups = fs::read_dir(layout.data_directory())
            .expect("data directory should be readable")
            .filter_map(std::result::Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .contains("pre-migration")
            })
            .count();
        assert_eq!(backups, 1);
    }

    #[test]
    fn normalized_records_and_cursor_commit_or_roll_back_together() {
        let temporary = tempdir().expect("temporary directory should be created");
        let layout = StoreLayout::new(temporary.path().join("intuigram"));
        let database =
            AccountDatabase::begin_login(&layout).expect("pending login database should open");
        let mut invalid = sync_batch();
        invalid.messages[0].chat_id = 999;

        assert!(database.commit_sync(invalid).is_err());
        assert_eq!(
            database
                .cached_account()
                .expect("rolled-back cache should load"),
            super::CachedAccount::default()
        );

        database
            .commit_sync(sync_batch())
            .expect("valid synchronized records should commit");
        let cached = database
            .cached_account()
            .expect("committed cache should load");
        assert_eq!(cached.cursors, vec![sync_batch().cursor]);
        assert_eq!(cached.folders, sync_batch().folders);
        assert_eq!(cached.chats, sync_batch().chats);
        assert_eq!(cached.messages, sync_batch().messages);
    }

    #[test]
    fn replacing_a_draft_keeps_the_current_value_durable() {
        let temporary = tempdir().expect("temporary directory should be created");
        let layout = StoreLayout::new(temporary.path().join("intuigram"));
        let database =
            AccountDatabase::begin_login(&layout).expect("pending login database should open");
        database
            .save_draft(StoredDraft {
                chat_id: 7,
                thread_root: None,
                text: "first".to_owned(),
                reply_to: None,
                modified_at: 10,
            })
            .expect("initial Draft should persist");
        let replacement = StoredDraft {
            chat_id: 7,
            thread_root: None,
            text: "second".to_owned(),
            reply_to: Some(3),
            modified_at: 20,
        };

        database
            .save_draft(replacement.clone())
            .expect("replacement Draft should persist");

        assert_eq!(
            database
                .cached_account()
                .expect("Draft cache should load")
                .drafts,
            vec![replacement]
        );
    }

    fn sync_batch() -> SyncBatch {
        SyncBatch {
            cursor: SyncCursor {
                scope: "account".to_owned(),
                pts: 12,
                qts: 0,
                date: 34,
                seq: 5,
            },
            folders: vec![StoredFolder {
                id: 0,
                title: "All".to_owned(),
                unread: 1,
            }],
            chats: vec![StoredChat {
                id: 7,
                kind: "private".to_owned(),
                title: "Ada".to_owned(),
                preview: "hello".to_owned(),
                unread: 1,
                pinned: false,
                folders: vec![0],
            }],
            messages: vec![StoredMessage {
                chat_id: 7,
                id: 42,
                sender: "Ada".to_owned(),
                body: "hello".to_owned(),
                timestamp: "12:00".to_owned(),
                direction: "incoming".to_owned(),
                delivery: "sent".to_owned(),
                reply_to: None,
                thread_root: Some(41),
                content_kind: "text".to_owned(),
                metadata: String::new(),
            }],
        }
    }
}
