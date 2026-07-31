use std::fs;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::{self, JoinHandle};

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
    /// This build cannot enforce owner-only permissions on the platform.
    #[snafu(display("owner-only database permissions are unsupported on this platform"))]
    UnsupportedPermissions,
}

/// Result returned by account database operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

enum Command {
    ReadIdentity {
        reply: SyncSender<Result<Option<AccountId>>>,
    },
    WriteIdentity {
        account: AccountId,
        reply: SyncSender<Result<()>>,
    },
    Shutdown,
}

/// A database containing one Telegram account's durable state.
pub struct AccountDatabase {
    commands: SyncSender<Command>,
    worker: Option<JoinHandle<()>>,
}

impl AccountDatabase {
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

    fn spawn(path: PathBuf, create: bool) -> Result<Self> {
        prepare_data_directory(&path)?;
        let (commands, requests) = mpsc::sync_channel(32);
        let (ready, initialized) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("popgram-account-db".to_owned())
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

#[cfg(test)]
mod tests {
    use std::fs;

    use rusqlite::Connection;
    use tempfile::tempdir;

    use super::AccountDatabase;
    use crate::{AccountId, StoreLayout};

    #[test]
    fn pending_login_is_promoted_to_a_persistent_account_database() {
        let temporary = tempdir().expect("temporary directory should be created");
        let layout = StoreLayout::new(temporary.path().join("popgram"));
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
    fn opening_a_missing_account_does_not_create_a_database() {
        let temporary = tempdir().expect("temporary directory should be created");
        let layout = StoreLayout::new(temporary.path().join("popgram"));
        let account = AccountId::new(7).expect("fixture ID should be positive");

        assert!(AccountDatabase::open(&layout, account).is_err());
        assert!(!layout.account_database(account).exists());
    }

    #[test]
    fn promotion_never_replaces_an_existing_account_database() {
        let temporary = tempdir().expect("temporary directory should be created");
        let layout = StoreLayout::new(temporary.path().join("popgram"));
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
        let layout = StoreLayout::new(temporary.path().join("popgram"));
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
}
