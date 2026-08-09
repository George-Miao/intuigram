use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use snafu::ResultExt;

use super::error::{
    InvalidAuthorizationKeySnafu, ReadOutboxRecordsSnafu, ReadUniqueRecordsSnafu, RecoveryError,
    RecoveryResult,
};
use super::types::{DraftHistory, UniqueRecords};
use crate::{AccountCipher, AccountId, SessionMaterial, StoredDraft};

pub(super) fn read_unique_records(
    path: &Path,
    expected: AccountId,
    cipher: &AccountCipher,
) -> RecoveryResult<UniqueRecords> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).context(
        ReadUniqueRecordsSnafu {
            path: path.to_path_buf(),
        },
    )?;
    if let Some(pragma) = cipher.key_pragma() {
        connection
            .execute_batch(&pragma)
            .context(ReadUniqueRecordsSnafu {
                path: path.to_path_buf(),
            })?;
    }
    let account = connection
        .query_row(
            "SELECT telegram_user_id FROM account_identity WHERE singleton = 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .context(ReadUniqueRecordsSnafu {
            path: path.to_path_buf(),
        })?;
    let account = AccountId::new(account)
        .filter(|account| *account == expected)
        .ok_or_else(|| RecoveryError::ReadUniqueRecords {
            path: path.to_path_buf(),
            source: rusqlite::Error::InvalidQuery,
        })?;
    Ok(UniqueRecords {
        account,
        session: read_session(&connection, path)?,
        drafts: read_drafts(&connection, path)?,
        draft_history: read_draft_history(&connection, path)?,
        outbox: crate::account::outbox::load(&connection).context(ReadOutboxRecordsSnafu {
            path: path.to_path_buf(),
        })?,
    })
}

fn read_session(connection: &Connection, path: &Path) -> RecoveryResult<Option<SessionMaterial>> {
    let row = connection
        .query_row(
            "SELECT dc_id, endpoint, auth_key, time_offset, first_salt FROM mtproto_session WHERE \
             singleton = 1",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()
        .context(ReadUniqueRecordsSnafu {
            path: path.to_path_buf(),
        })?;
    row.map(|(dc_id, endpoint, key, time_offset, first_salt)| {
        let length = key.len();
        let auth_key: [u8; 256] = key.try_into().map_err(|_| {
            InvalidAuthorizationKeySnafu {
                path: path.to_path_buf(),
                length,
            }
            .build()
        })?;
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

fn read_drafts(connection: &Connection, path: &Path) -> RecoveryResult<Vec<StoredDraft>> {
    let mut statement = connection
        .prepare(
            "SELECT chat_id, thread_root_message_id, saved_peer_id, text, reply_to_message_id, \
             modified_at FROM drafts ORDER BY chat_id, thread_root_message_id, saved_peer_id",
        )
        .context(ReadUniqueRecordsSnafu {
            path: path.to_path_buf(),
        })?;
    statement
        .query_map([], |row| {
            let thread: i64 = row.get(1)?;
            Ok(StoredDraft {
                chat_id: row.get(0)?,
                thread_root: (thread != 0).then_some(thread),
                saved_peer: match row.get::<_, i64>(2)? {
                    0 => None,
                    peer => Some(peer),
                },
                text: row.get(3)?,
                reply_to: row.get(4)?,
                modified_at: row.get(5)?,
            })
        })
        .and_then(Iterator::collect)
        .context(ReadUniqueRecordsSnafu {
            path: path.to_path_buf(),
        })
}

fn read_draft_history(connection: &Connection, path: &Path) -> RecoveryResult<Vec<DraftHistory>> {
    let mut statement = connection
        .prepare(
            "SELECT chat_id, thread_root_message_id, saved_peer_id, text, reply_to_message_id, \
             displaced_at FROM draft_history ORDER BY version_id",
        )
        .context(ReadUniqueRecordsSnafu {
            path: path.to_path_buf(),
        })?;
    statement
        .query_map([], |row| {
            let thread: i64 = row.get(1)?;
            Ok(DraftHistory {
                chat_id: row.get(0)?,
                thread_root: (thread != 0).then_some(thread),
                saved_peer: match row.get::<_, i64>(2)? {
                    0 => None,
                    peer => Some(peer),
                },
                text: row.get(3)?,
                reply_to: row.get(4)?,
                displaced_at: row.get(5)?,
            })
        })
        .and_then(Iterator::collect)
        .context(ReadUniqueRecordsSnafu {
            path: path.to_path_buf(),
        })
}

pub(super) fn discover_backups(path: &Path) -> Vec<PathBuf> {
    let Some(directory) = path.parent() else {
        return Vec::new();
    };
    let prefix = path.file_name().unwrap_or_default().to_string_lossy();
    let mut backups = fs::read_dir(directory)
        .into_iter()
        .flatten()
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|candidate| {
            let name = candidate.file_name().unwrap_or_default().to_string_lossy();
            name.starts_with(prefix.as_ref()) && name.ends_with(".bak")
        })
        .collect::<Vec<_>>();
    backups.sort();
    backups
}
