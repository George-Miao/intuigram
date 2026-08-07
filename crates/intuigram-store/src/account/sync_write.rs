pub(super) fn commit_sync(connection: &Connection, batch: SyncBatch) -> Result<()> {
    let transaction = connection
        .unchecked_transaction()
        .context(CommitSyncSnafu)?;
    for cursor in batch.cursors {
        transaction
            .execute(
                "INSERT INTO sync_state(scope, pts, qts, date, seq) VALUES (?1, ?2, ?3, ?4, ?5) \
                 ON CONFLICT(scope) DO UPDATE SET pts=excluded.pts, qts=excluded.qts, \
                 date=excluded.date, seq=excluded.seq",
                params![
                    cursor.scope,
                    cursor.pts,
                    cursor.qts,
                    cursor.date,
                    cursor.seq
                ],
            )
            .context(CommitSyncSnafu)?;
    }
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
    for mutation in batch.mutations {
        apply_sync_mutation(&transaction, mutation).context(CommitSyncSnafu)?;
    }
    transaction.commit().context(CommitSyncSnafu)
}

pub(super) fn apply_sync_mutation(
    connection: &Connection,
    mutation: StoredMutation,
) -> rusqlite::Result<()> {
    match mutation {
        StoredMutation::SetMessagesPinned {
            chat_id,
            ids,
            pinned,
        } => {
            for id in ids {
                connection.execute(
                    "UPDATE messages SET metadata = json_set(metadata, '$.pinned', CASE WHEN ?3 \
                     THEN json('true') ELSE json('false') END) WHERE chat_id = ?1 AND message_id \
                     = ?2",
                    params![chat_id, id, pinned],
                )?;
            }
        }
        StoredMutation::DeleteMessages { chat_id, ids } => {
            for id in ids {
                if let Some(chat_id) = chat_id {
                    connection.execute(
                        "DELETE FROM messages WHERE chat_id = ?1 AND message_id = ?2",
                        params![chat_id, id],
                    )?;
                } else {
                    connection.execute(
                        "DELETE FROM messages WHERE message_id = ?1 AND chat_id IN (SELECT \
                         chat_id FROM chats WHERE kind NOT IN ('channel', 'supergroup', \
                         'gigagroup'))",
                        [id],
                    )?;
                }
            }
        }
        StoredMutation::ReadHistory {
            chat_id,
            max_id,
            outgoing,
            unread,
        } => {
            if outgoing {
                connection.execute(
                    "UPDATE messages SET delivery = 'read' WHERE chat_id = ?1 AND message_id <= \
                     ?2 AND direction = 'outgoing'",
                    params![chat_id, max_id],
                )?;
            }
            if let Some(unread) = unread {
                connection.execute(
                    "UPDATE chats SET unread_count = ?2 WHERE chat_id = ?1",
                    params![chat_id, unread],
                )?;
            }
        }
        StoredMutation::MoveArchive { chat_id, archived } => {
            connection.execute(
                "DELETE FROM chat_folders WHERE chat_id = ?1 AND folder_id IN (0, -1)",
                [chat_id],
            )?;
            connection.execute(
                "INSERT INTO chat_folders(chat_id, folder_id, position) VALUES (?1, ?2, 0)",
                params![chat_id, if archived { -1 } else { 0 }],
            )?;
        }
    }
    Ok(())
}
use super::*;
