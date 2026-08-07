//! Cross-Account registry persistence.

use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::{self, JoinHandle};
use std::{fs, io};

use rusqlite::backup::Backup;
use rusqlite::{Connection, OpenFlags, params};
use snafu::ResultExt;

use crate::{AccountId, StoreLayout};

mod migrations {
    refinery::embed_migrations!("migrations/global");
}

mod error;

#[cfg(not(unix))]
use error::UnsupportedPermissionsSnafu;
use error::{
    BackupDatabaseSnafu, BackupNamesExhaustedSnafu, CreateDataDirectorySnafu,
    DatabaseCheckFailedSnafu, InspectMigrationsSnafu, ListAccountsSnafu, MigrateDatabaseSnafu,
    OpenDatabaseSnafu, ProtectDataPathSnafu, RegisterAccountSnafu, RemoveAccountSnafu,
    RunDatabaseCheckSnafu, SpawnWorkerSnafu,
};
pub use error::{Error, Result};

/// Cross-Account metadata retained in `global.db`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountRecord {
    /// Telegram user ID and Account database identity.
    pub id: AccountId,
    /// Last synchronized display name.
    pub display_name: String,
    /// Whether Intuigram should open this Account at startup.
    pub active: bool,
}

enum Command {
    Register {
        account: AccountRecord,
        reply: SyncSender<Result<()>>,
    },
    List {
        reply: SyncSender<Result<Vec<AccountRecord>>>,
    },
    Remove {
        account: AccountId,
        reply: SyncSender<Result<()>>,
    },
    Shutdown,
}

/// Dedicated-thread interface to cross-Account metadata.
pub struct GlobalDatabase {
    commands: SyncSender<Command>,
    worker: Option<JoinHandle<()>>,
}

impl GlobalDatabase {
    /// Opens and migrates `global.db` on its database worker.
    pub fn open(layout: &StoreLayout) -> Result<Self> {
        let path = layout.global_database();
        prepare_data_directory(&path)?;
        let (commands, requests) = mpsc::sync_channel(32);
        let (ready, initialized) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("intuigram-global-db".to_owned())
            .spawn(move || run_worker(&path, &requests, &ready))
            .context(SpawnWorkerSnafu)?;
        initialized.recv().map_err(|_| Error::WorkerUnavailable)??;
        Ok(Self {
            commands,
            worker: Some(worker),
        })
    }

    /// Inserts or updates an Account and optionally makes it active.
    pub fn register(&self, account: AccountRecord) -> Result<()> {
        let (reply, response) = mpsc::sync_channel(1);
        self.commands
            .send(Command::Register { account, reply })
            .map_err(|_| Error::WorkerUnavailable)?;
        response.recv().map_err(|_| Error::WorkerUnavailable)?
    }

    /// Returns registered Accounts with the active Account first.
    pub fn accounts(&self) -> Result<Vec<AccountRecord>> {
        let (reply, response) = mpsc::sync_channel(1);
        self.commands
            .send(Command::List { reply })
            .map_err(|_| Error::WorkerUnavailable)?;
        response.recv().map_err(|_| Error::WorkerUnavailable)?
    }

    /// Removes one Account from the cross-Account registry.
    pub fn remove(&self, account: AccountId) -> Result<()> {
        let (reply, response) = mpsc::sync_channel(1);
        self.commands
            .send(Command::Remove { account, reply })
            .map_err(|_| Error::WorkerUnavailable)?;
        response.recv().map_err(|_| Error::WorkerUnavailable)?
    }
}

impl Drop for GlobalDatabase {
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

fn run_worker(path: &Path, requests: &Receiver<Command>, ready: &SyncSender<Result<()>>) {
    let connection = open_and_migrate(path);
    let Ok(mut connection) = connection else {
        let _ = ready.send(connection.map(|_| ()));
        return;
    };
    if ready.send(Ok(())).is_err() {
        return;
    }

    while let Ok(command) = requests.recv() {
        match command {
            Command::Register { account, reply } => {
                let result = register_account(&mut connection, &account);
                let _ = reply.send(result);
            }
            Command::List { reply } => {
                let _ = reply.send(list_accounts(&connection));
            }
            Command::Remove { account, reply } => {
                let result = remove_account(&mut connection, account);
                let _ = reply.send(result);
            }
            Command::Shutdown => break,
        }
    }
}

fn remove_account(connection: &mut Connection, account: AccountId) -> Result<()> {
    let transaction = connection
        .transaction()
        .context(RemoveAccountSnafu { account })?;
    transaction
        .execute(
            "DELETE FROM accounts WHERE telegram_user_id = ?1",
            [account.get()],
        )
        .context(RemoveAccountSnafu { account })?;
    transaction
        .execute(
            "UPDATE accounts SET active = 1 WHERE telegram_user_id = (SELECT telegram_user_id \
             FROM accounts ORDER BY last_used_at DESC, telegram_user_id LIMIT 1) AND NOT EXISTS \
             (SELECT 1 FROM accounts WHERE active = 1)",
            [],
        )
        .context(RemoveAccountSnafu { account })?;
    transaction.commit().context(RemoveAccountSnafu { account })
}

fn open_and_migrate(path: &Path) -> Result<Connection> {
    let existed = path.is_file();
    let mut connection = Connection::open(path).context(OpenDatabaseSnafu {
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
    validate_database(&connection, path)?;
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
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => continue,
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

fn validate_database(connection: &Connection, path: &Path) -> Result<()> {
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
    Ok(())
}

fn register_account(connection: &mut Connection, account: &AccountRecord) -> Result<()> {
    let transaction = connection.transaction().context(RegisterAccountSnafu {
        account: account.id,
    })?;
    if account.active {
        transaction
            .execute("UPDATE accounts SET active = 0 WHERE active = 1", [])
            .context(RegisterAccountSnafu {
                account: account.id,
            })?;
    }
    transaction
        .execute(
            "INSERT INTO accounts (telegram_user_id, display_name, active, last_used_at) VALUES \
             (?1, ?2, ?3, unixepoch()) ON CONFLICT(telegram_user_id) DO UPDATE SET display_name = \
             excluded.display_name, active = excluded.active, last_used_at = excluded.last_used_at",
            params![account.id.get(), account.display_name, account.active],
        )
        .context(RegisterAccountSnafu {
            account: account.id,
        })?;
    transaction.commit().context(RegisterAccountSnafu {
        account: account.id,
    })
}

fn list_accounts(connection: &Connection) -> Result<Vec<AccountRecord>> {
    let mut statement = connection
        .prepare(
            "SELECT telegram_user_id, display_name, active FROM accounts ORDER BY active DESC, \
             last_used_at DESC, telegram_user_id",
        )
        .context(ListAccountsSnafu)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, bool>(2)?,
            ))
        })
        .context(ListAccountsSnafu)?;
    rows.map(|row| {
        let (raw_id, display_name, active) = row.context(ListAccountsSnafu)?;
        let id = AccountId::new(raw_id).ok_or(Error::InvalidAccountId { value: raw_id })?;
        Ok(AccountRecord {
            id,
            display_name,
            active,
        })
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rusqlite::Connection;
    use tempfile::tempdir;

    use super::{AccountRecord, GlobalDatabase};
    use crate::{AccountId, StoreLayout};

    #[test]
    fn account_registry_persists_one_active_account() {
        let temporary = tempdir().expect("temporary directory should be created");
        let layout = StoreLayout::new(temporary.path().join("intuigram"));
        let first = AccountId::new(11).expect("fixture ID should be positive");
        let second = AccountId::new(22).expect("fixture ID should be positive");
        let database = GlobalDatabase::open(&layout).expect("global database should open");

        database
            .register(AccountRecord {
                id: first,
                display_name: "First".to_owned(),
                active: true,
            })
            .expect("first Account should register");
        database
            .register(AccountRecord {
                id: second,
                display_name: "Second".to_owned(),
                active: true,
            })
            .expect("second Account should register");
        drop(database);

        let reopened = GlobalDatabase::open(&layout).expect("global database should reopen");
        let accounts = reopened.accounts().expect("Accounts should load");
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].id, second);
        assert!(accounts[0].active);
        assert!(!accounts[1].active);
    }

    #[test]
    fn removing_an_account_leaves_other_registry_records() {
        let temporary = tempdir().expect("temporary directory should be created");
        let layout = StoreLayout::new(temporary.path().join("intuigram"));
        let first = AccountId::new(11).expect("fixture ID should be positive");
        let second = AccountId::new(22).expect("fixture ID should be positive");
        let database = GlobalDatabase::open(&layout).expect("global database should open");
        for id in [first, second] {
            database
                .register(AccountRecord {
                    id,
                    display_name: id.get().to_string(),
                    active: id == first,
                })
                .expect("account should register");
        }

        database.remove(first).expect("account should be removed");

        assert_eq!(
            database
                .accounts()
                .expect("remaining accounts should load")
                .into_iter()
                .map(|account| account.id)
                .collect::<Vec<_>>(),
            vec![second]
        );
        assert!(
            database
                .accounts()
                .expect("remaining Account should load")
                .first()
                .is_some_and(|account| account.active)
        );
    }

    #[test]
    fn an_existing_global_database_is_backed_up_before_migration() {
        let temporary = tempdir().expect("temporary directory should be created");
        let layout = StoreLayout::new(temporary.path().join("intuigram"));
        fs::create_dir_all(layout.data_directory()).expect("data directory should be created");
        let path = layout.global_database();
        let connection = Connection::open(&path).expect("fixture database should open");
        connection
            .execute("CREATE TABLE legacy(value TEXT NOT NULL)", [])
            .expect("legacy schema should be created");
        drop(connection);

        let database = GlobalDatabase::open(&layout).expect("global database should migrate");
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
