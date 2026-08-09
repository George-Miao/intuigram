pub(super) fn prepare_data_directory(database: &Path) -> Result<()> {
    let directory = database.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(directory).context(CreateDataDirectorySnafu {
        path: directory.to_path_buf(),
    })?;
    protect_path(directory, true)
}

#[cfg(unix)]
pub(super) fn protect_path(path: &Path, directory: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mode = if directory { 0o700 } else { 0o600 };
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).context(ProtectDataPathSnafu {
        path: path.to_path_buf(),
    })
}

#[cfg(not(unix))]
pub(super) fn protect_path(_path: &Path, _directory: bool) -> Result<()> {
    UnsupportedPermissionsSnafu.fail()
}

#[cfg(any(unix, target_os = "wasi"))]
pub(super) fn promote_without_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};

    renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE).map_err(std::io::Error::from)
}

#[cfg(not(any(unix, target_os = "wasi")))]
pub(super) fn promote_without_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::rename(from, to)
}

pub(super) fn run_worker(
    path: &Path,
    create: bool,
    cipher: AccountCipher,
    requests: &Receiver<Command>,
    ready: &SyncSender<Result<()>>,
) {
    let connection = open_and_migrate(path, create, &cipher);
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
                            session.auth_key().as_slice(),
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
            Command::SaveSelection { selection, reply } => {
                let _ = reply.send(save_selection(&connection, selection));
            }
            Command::SetChatMediaOffline {
                chat_id,
                keep,
                reply,
            } => {
                let _ = reply.send(set_chat_media_offline(&connection, chat_id, keep));
            }
            Command::SaveTopics {
                chat,
                topics,
                reply,
            } => {
                let _ = reply.send(save_topics(&connection, chat, topics));
            }
            Command::CommitSyncAsync { batch, reply } => {
                reply.finish(commit_sync(&connection, *batch));
            }
            Command::SaveDraftAsync { draft, reply } => {
                reply.finish(save_draft(&connection, draft));
            }
            Command::SaveSelectionAsync { selection, reply } => {
                reply.finish(save_selection(&connection, selection));
            }
            Command::SetChatMediaOfflineAsync {
                chat_id,
                keep,
                reply,
            } => {
                reply.finish(set_chat_media_offline(&connection, chat_id, keep));
            }
            Command::SaveMessagesAsync { messages, reply } => {
                reply.finish(save_messages(&connection, messages));
            }
            Command::SaveTopicsAsync {
                chat,
                topics,
                reply,
            } => {
                reply.finish(save_topics(&connection, chat, topics));
            }
            Command::ReplaceMessageAsync {
                chat,
                local_id,
                message,
                reply,
            } => {
                reply.finish(replace_message(&connection, chat, local_id, *message));
            }
            Command::SaveChatHistoryAsync {
                chat,
                messages,
                pinned_messages,
                status,
                reply,
            } => reply.finish(save_chat_history(
                &connection,
                chat,
                messages,
                pinned_messages,
                status,
            )),
            Command::DeleteMessagesAsync {
                chat,
                messages,
                reply,
            } => {
                reply.finish(delete_messages(&connection, chat, messages));
            }
            Command::Shutdown => break,
        }
    }
}
use super::*;
