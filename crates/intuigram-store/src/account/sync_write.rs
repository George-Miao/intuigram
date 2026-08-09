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
                "INSERT INTO chats(chat_id, kind, title, preview, status, unread_count, pinned, \
                 can_pin_messages, has_topics, has_direct_messages) VALUES (?1, ?2, ?3, ?4, ?5, \
                 ?6, ?7, ?8, ?9, ?10) ON CONFLICT(chat_id) DO UPDATE SET kind=excluded.kind, \
                 title=excluded.title, preview=excluded.preview, status=excluded.status, \
                 unread_count=excluded.unread_count, pinned=excluded.pinned, \
                 can_pin_messages=excluded.can_pin_messages, has_topics=excluded.has_topics, \
                 has_direct_messages=excluded.has_direct_messages",
                params![
                    chat.id,
                    chat.kind,
                    chat.title,
                    chat.preview,
                    chat.status,
                    chat.unread,
                    chat.pinned,
                    chat.can_pin_messages,
                    chat.has_topics,
                    chat.has_direct_messages,
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
    if let Some(order) = batch.chat_order {
        // Only a complete bootstrap can define every Chat's relative position;
        // incremental record upserts must leave the last authoritative order intact.
        transaction
            .execute("UPDATE chats SET position = NULL", [])
            .context(CommitSyncSnafu)?;
        for (position, chat) in order.into_iter().enumerate() {
            let position = i64::try_from(position)
                .expect("an in-memory Chat list cannot exceed SQLite's signed index range");
            transaction
                .execute(
                    "UPDATE chats SET position = ?2 WHERE chat_id = ?1",
                    params![chat, position],
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
        StoredMutation::SetChatPinPermission {
            chat_id,
            can_pin_messages,
        } => {
            connection.execute(
                "UPDATE chats SET can_pin_messages = ?2 WHERE chat_id = ?1",
                params![chat_id, can_pin_messages],
            )?;
        }
        StoredMutation::SetChatHasTopics {
            chat_id,
            has_topics,
        } => {
            connection.execute(
                "UPDATE chats SET has_topics = ?2 WHERE chat_id = ?1",
                params![chat_id, has_topics],
            )?;
        }
        StoredMutation::SetChatHasDirectMessages {
            chat_id,
            has_direct_messages,
        } => {
            connection.execute(
                "UPDATE chats SET has_direct_messages = ?2 WHERE chat_id = ?1",
                params![chat_id, has_direct_messages],
            )?;
        }
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
                if pinned {
                    connection.execute(
                        "INSERT OR IGNORE INTO pinned_message_projection(chat_id, message_id) \
                         SELECT ?1, ?2 WHERE EXISTS (SELECT 1 FROM messages WHERE chat_id = ?1 \
                         AND message_id = ?2)",
                        params![chat_id, id],
                    )?;
                } else {
                    connection.execute(
                        "DELETE FROM pinned_message_projection WHERE chat_id = ?1 AND message_id \
                         = ?2",
                        params![chat_id, id],
                    )?;
                }
            }
        }
        StoredMutation::SetPaidMediaItems {
            chat_id,
            message_id,
            items,
        } => {
            // Partial Telegram updates do not contain the Stars price or the
            // enclosing Message. The generic upsert API therefore cannot
            // express this lossless child replacement; update only the typed
            // JSON path while retaining every surrounding field.
            connection.execute(
                "UPDATE messages SET metadata = json_set(metadata, '$.media.specialized.items', \
                 json(?3)) WHERE chat_id = ?1 AND message_id = ?2",
                params![chat_id, message_id, items],
            )?;
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
            saved_peer,
            max_id,
            outgoing,
            unread,
        } => {
            if outgoing {
                connection.execute(
                    "UPDATE messages SET delivery = 'read' WHERE chat_id = ?1 AND message_id <= \
                     ?2 AND saved_peer_id IS ?3 AND direction = 'outgoing'",
                    params![chat_id, max_id, saved_peer],
                )?;
            }
            if let Some(unread) = unread {
                if let Some(saved_peer) = saved_peer {
                    connection.execute(
                        "UPDATE saved_dialogs SET unread_count = ?3, unread_mark = 0 WHERE \
                         chat_id = ?1 AND saved_peer_id = ?2",
                        params![chat_id, saved_peer, unread],
                    )?;
                } else {
                    connection.execute(
                        "UPDATE chats SET unread_count = ?2 WHERE chat_id = ?1",
                        params![chat_id, unread],
                    )?;
                }
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
