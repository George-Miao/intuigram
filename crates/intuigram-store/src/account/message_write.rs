pub(super) fn save_messages(connection: &Connection, messages: Vec<StoredMessage>) -> Result<()> {
    let transaction = connection
        .unchecked_transaction()
        .context(SaveMessagesSnafu)?;
    for message in messages {
        upsert_message(&transaction, &message).context(SaveMessagesSnafu)?;
    }
    transaction.commit().context(SaveMessagesSnafu)
}

pub(super) fn replace_message(
    connection: &Connection,
    chat: i64,
    local_id: i64,
    message: StoredMessage,
) -> Result<()> {
    let transaction = connection
        .unchecked_transaction()
        .context(SaveMessagesSnafu)?;
    upsert_message(&transaction, &message).context(SaveMessagesSnafu)?;
    apply_sync_mutation(
        &transaction,
        StoredMutation::DeleteMessages {
            chat_id: Some(chat),
            ids: vec![local_id],
        },
    )
    .context(SaveMessagesSnafu)?;
    transaction.commit().context(SaveMessagesSnafu)
}

pub(super) fn save_chat_history(
    connection: &Connection,
    chat: i64,
    messages: Vec<StoredMessage>,
    pinned_messages: Vec<StoredMessage>,
) -> Result<()> {
    let transaction = connection
        .unchecked_transaction()
        .context(SaveMessagesSnafu)?;
    let recent_ids = messages
        .iter()
        .map(|message| message.id)
        .collect::<Vec<_>>();
    let oldest_recent_id = recent_ids.iter().copied().filter(|id| *id > 0).min();
    for message in messages.iter().chain(&pinned_messages) {
        upsert_message(&transaction, message).context(SaveMessagesSnafu)?;
    }
    for id in &recent_ids {
        transaction
            .execute(
                "DELETE FROM message_projections WHERE chat_id = ?1 AND message_id = ?2",
                params![chat, id],
            )
            .context(SaveMessagesSnafu)?;
    }
    transaction
        .execute(
            "DELETE FROM pinned_message_projection WHERE chat_id = ?1",
            [chat],
        )
        .context(SaveMessagesSnafu)?;
    for message in pinned_messages {
        transaction
            .execute(
                "INSERT INTO pinned_message_projection(chat_id, message_id) VALUES (?1, ?2)",
                params![chat, message.id],
            )
            .context(SaveMessagesSnafu)?;
        if !recent_ids.contains(&message.id) {
            transaction
                .execute(
                    "INSERT OR IGNORE INTO message_projections(chat_id, message_id) VALUES (?1, \
                     ?2)",
                    params![chat, message.id],
                )
                .context(SaveMessagesSnafu)?;
        }
    }
    let stale_ids = super::cache_read::query_messages(&transaction)
        .context(SaveMessagesSnafu)?
        .into_iter()
        .filter(|message| {
            message.chat_id == chat
                && message.thread_root.is_none()
                && message.id > 0
                && matches!(message.delivery.as_str(), "sent" | "read")
                && !recent_ids.contains(&message.id)
                && oldest_recent_id.is_none_or(|oldest| message.id >= oldest)
        })
        .map(|message| message.id)
        .collect::<Vec<_>>();
    if !stale_ids.is_empty() {
        apply_sync_mutation(
            &transaction,
            StoredMutation::DeleteMessages {
                chat_id: Some(chat),
                ids: stale_ids,
            },
        )
        .context(SaveMessagesSnafu)?;
    }
    transaction.commit().context(SaveMessagesSnafu)
}

pub(super) fn delete_messages(
    connection: &Connection,
    chat: Option<i64>,
    messages: Vec<i64>,
) -> Result<()> {
    let transaction = connection
        .unchecked_transaction()
        .context(SaveMessagesSnafu)?;
    apply_sync_mutation(
        &transaction,
        StoredMutation::DeleteMessages {
            chat_id: chat,
            ids: messages,
        },
    )
    .context(SaveMessagesSnafu)?;
    transaction.commit().context(SaveMessagesSnafu)
}

pub(super) fn upsert_message(
    connection: &Connection,
    message: &StoredMessage,
) -> rusqlite::Result<()> {
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

pub(super) fn save_draft(connection: &Connection, draft: StoredDraft) -> Result<()> {
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
use super::*;
