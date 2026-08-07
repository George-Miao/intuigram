pub(super) fn load_cache(connection: &Connection) -> Result<CachedAccount> {
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
    let pinned_messages = load_pinned_messages(connection)?;
    let drafts = load_drafts(connection)?;
    let selection = connection
        .query_row(
            "SELECT folder_id, chat_id FROM ui_selection WHERE singleton = 1",
            [],
            |row| {
                Ok(StoredSelection {
                    folder_id: row.get(0)?,
                    chat_id: row.get(1)?,
                })
            },
        )
        .optional()
        .context(LoadCacheSnafu)?;
    Ok(CachedAccount {
        cursors,
        folders,
        chats,
        messages,
        pinned_messages,
        drafts,
        selection,
    })
}

pub(super) fn load_cursors(connection: &Connection) -> Result<Vec<SyncCursor>> {
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

pub(super) fn load_folders(connection: &Connection) -> Result<Vec<StoredFolder>> {
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

pub(super) fn load_chats(connection: &Connection) -> Result<Vec<StoredChat>> {
    let mut statement = connection
        .prepare(
            "SELECT chat_id, kind, title, preview, status, unread_count, pinned, can_pin_messages \
             FROM chats ORDER BY pinned DESC, chat_id DESC",
        )
        .context(LoadCacheSnafu)?;
    statement
        .query_map([], |row| {
            let unread = row.get::<_, i64>(5)?;
            Ok(StoredChat {
                id: row.get(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                preview: row.get(3)?,
                status: row.get(4)?,
                unread: u32::try_from(unread).unwrap_or(0),
                pinned: row.get(6)?,
                can_pin_messages: row.get(7)?,
                folders: Vec::new(),
            })
        })
        .context(LoadCacheSnafu)?
        .collect::<std::result::Result<_, _>>()
        .context(LoadCacheSnafu)
}

pub(super) fn load_messages(connection: &Connection) -> Result<Vec<StoredMessage>> {
    let mut statement = connection
        .prepare(
            "SELECT chat_id, message_id, sender, body, timestamp, direction, delivery, \
             reply_to_message_id, thread_root_message_id, content_kind, metadata FROM messages m \
             WHERE NOT EXISTS (SELECT 1 FROM message_projections p WHERE p.chat_id = m.chat_id \
             AND p.message_id = m.message_id) ORDER BY m.chat_id, m.message_id",
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

pub(super) fn load_pinned_messages(connection: &Connection) -> Result<Vec<StoredMessage>> {
    let mut statement = connection
        .prepare(
            "SELECT m.chat_id, m.message_id, m.sender, m.body, m.timestamp, m.direction, \
             m.delivery, m.reply_to_message_id, m.thread_root_message_id, m.content_kind, \
             m.metadata FROM messages m JOIN pinned_message_projection p ON p.chat_id = m.chat_id \
             AND p.message_id = m.message_id ORDER BY m.chat_id, m.message_id",
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

pub(super) fn load_drafts(connection: &Connection) -> Result<Vec<StoredDraft>> {
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
use super::*;
