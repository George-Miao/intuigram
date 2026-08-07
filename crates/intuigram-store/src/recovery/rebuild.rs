use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};
use snafu::ResultExt;

use super::error::{
    CopyUniqueRecordsSnafu, CreateRebuiltDatabaseSnafu, OpenRebuiltDatabaseSnafu,
    PreserveOriginalSnafu, RebuildNamesExhaustedSnafu, RecoveryError, RecoveryResult,
};
use super::types::{RebuiltAccount, UniqueRecords};
use crate::{AccountCipher, AccountDatabase, AccountId, StoreLayout, account};

pub(super) fn rebuild(
    layout: StoreLayout,
    account: AccountId,
    cipher: AccountCipher,
    path: PathBuf,
    records: UniqueRecords,
) -> RecoveryResult<RebuiltAccount> {
    let workspace = RebuildWorkspace::reserve(&path)?;
    let mut connection = account::open_and_migrate(&workspace.database, true, &cipher)
        .map_err(Box::new)
        .context(CreateRebuiltDatabaseSnafu {
            path: workspace.database.clone(),
        })?;
    copy_unique_records(&mut connection, &workspace.database, records)?;
    drop(connection);

    let backup = available_backup_path(&path)?;
    rename_without_replace(&path, &backup).context(PreserveOriginalSnafu {
        path: path.clone(),
        backup: backup.clone(),
    })?;
    if let Err(source) = rename_without_replace(&workspace.database, &path) {
        let _ = fs::rename(&backup, &path);
        return Err(RecoveryError::InstallRebuiltDatabase { path, source });
    }
    let database = AccountDatabase::open_with_cipher(&layout, account, cipher)
        .map_err(Box::new)
        .context(OpenRebuiltDatabaseSnafu { path: path.clone() })?;
    Ok(RebuiltAccount {
        database,
        preserved_original: backup,
    })
}

fn copy_unique_records(
    connection: &mut Connection,
    path: &Path,
    records: UniqueRecords,
) -> RecoveryResult<()> {
    let transaction = connection.transaction().context(CopyUniqueRecordsSnafu {
        path: path.to_path_buf(),
    })?;
    transaction
        .execute(
            "INSERT INTO account_identity (singleton, telegram_user_id) VALUES (1, ?1)",
            params![records.account.get()],
        )
        .context(CopyUniqueRecordsSnafu {
            path: path.to_path_buf(),
        })?;
    if let Some(session) = records.session {
        transaction
            .execute(
                "INSERT INTO mtproto_session (singleton, dc_id, endpoint, auth_key, time_offset, \
                 first_salt) VALUES (1, ?1, ?2, ?3, ?4, ?5)",
                params![
                    session.dc_id,
                    session.endpoint,
                    session.auth_key().as_slice(),
                    session.time_offset,
                    session.first_salt
                ],
            )
            .context(CopyUniqueRecordsSnafu {
                path: path.to_path_buf(),
            })?;
    }
    for draft in records.drafts {
        transaction
            .execute(
                "INSERT INTO drafts (chat_id, thread_root_message_id, text, reply_to_message_id, \
                 modified_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    draft.chat_id,
                    draft.thread_root.unwrap_or(0),
                    draft.text,
                    draft.reply_to,
                    draft.modified_at
                ],
            )
            .context(CopyUniqueRecordsSnafu {
                path: path.to_path_buf(),
            })?;
    }
    for draft in records.draft_history {
        transaction
            .execute(
                "INSERT INTO draft_history (chat_id, thread_root_message_id, text, \
                 reply_to_message_id, displaced_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    draft.chat_id,
                    draft.thread_root.unwrap_or(0),
                    draft.text,
                    draft.reply_to,
                    draft.displaced_at
                ],
            )
            .context(CopyUniqueRecordsSnafu {
                path: path.to_path_buf(),
            })?;
    }
    transaction.commit().context(CopyUniqueRecordsSnafu {
        path: path.to_path_buf(),
    })
}

fn available_backup_path(path: &Path) -> RecoveryResult<PathBuf> {
    for attempt in 1..=1_000_u16 {
        let candidate = path.with_extension(format!("db.recovery-{attempt}.bak"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    RebuildNamesExhaustedSnafu {
        path: path.to_path_buf(),
    }
    .fail()
}

struct RebuildWorkspace {
    directory: PathBuf,
    database: PathBuf,
}

impl RebuildWorkspace {
    fn reserve(path: &Path) -> RecoveryResult<Self> {
        for attempt in 1..=1_000_u16 {
            let directory = path.with_extension(format!("db.rebuild-{attempt}.tmp"));
            match fs::create_dir(&directory) {
                Ok(()) => {
                    return Ok(Self {
                        database: directory.join("account.db"),
                        directory,
                    });
                }
                Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(source) => {
                    return Err(RecoveryError::ReserveRebuildWorkspace {
                        path: path.to_path_buf(),
                        source,
                    });
                }
            }
        }
        RebuildNamesExhaustedSnafu {
            path: path.to_path_buf(),
        }
        .fail()
    }
}

impl Drop for RebuildWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.database);
        let _ = fs::remove_dir(&self.directory);
    }
}

#[cfg(any(unix, target_os = "wasi"))]
fn rename_without_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};

    renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE).map_err(std::io::Error::from)
}

#[cfg(not(any(unix, target_os = "wasi")))]
fn rename_without_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::rename(from, to)
}
