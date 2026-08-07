pub(crate) fn open_and_migrate(
    path: &Path,
    create: bool,
    cipher: &AccountCipher,
) -> Result<Connection> {
    let existed = path.is_file();
    let mut flags = OpenFlags::SQLITE_OPEN_READ_WRITE;
    if create {
        flags |= OpenFlags::SQLITE_OPEN_CREATE;
    }
    let mut connection = Connection::open_with_flags(path, flags).context(OpenDatabaseSnafu {
        path: path.to_path_buf(),
    })?;
    if let Some(pragma) = cipher.key_pragma() {
        connection
            .execute_batch(&pragma)
            .context(OpenDatabaseSnafu {
                path: path.to_path_buf(),
            })?;
    }
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

pub(super) fn create_backup(source: &Connection, path: &Path) -> Result<PathBuf> {
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

pub(super) fn read_account_id(connection: &Connection) -> Result<Option<AccountId>> {
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
use super::*;
