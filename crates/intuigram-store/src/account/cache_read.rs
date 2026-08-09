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
    let topics = load_topics(connection)?;
    let saved_dialogs = load_saved_dialogs(connection)?;
    let messages = load_messages(connection)?;
    let pinned_messages = load_pinned_messages(connection)?;
    let drafts = load_drafts(connection)?;
    let transcript_anchors = load_transcript_anchors(connection).context(LoadCacheSnafu)?;
    let selection = connection
        .query_row(
            "SELECT folder_id, chat_id, anchor_message_id FROM ui_selection WHERE singleton = 1",
            [],
            |row| {
                Ok(StoredSelection {
                    folder_id: row.get(0)?,
                    chat_id: row.get(1)?,
                    anchor_message_id: row.get(2)?,
                    transcript_anchors: transcript_anchors.clone(),
                })
            },
        )
        .optional()
        .context(LoadCacheSnafu)?;
    let offline_chats = load_offline_chats(connection)?;
    Ok(CachedAccount {
        cursors,
        folders,
        chats,
        topics,
        saved_dialogs,
        messages,
        pinned_messages,
        drafts,
        selection,
        offline_chats,
    })
}

fn load_transcript_anchors(
    connection: &Connection,
) -> rusqlite::Result<Vec<StoredTranscriptAnchor>> {
    let mut statement = connection.prepare(
        "SELECT chat_id, thread_root_message_id, saved_peer_id, anchor_message_id FROM \
         transcript_anchors ORDER BY chat_id, thread_root_message_id, saved_peer_id",
    )?;
    statement
        .query_map([], |row| {
            let thread_root = row.get::<_, i64>(1)?;
            Ok(StoredTranscriptAnchor {
                chat_id: row.get(0)?,
                thread_root: (thread_root != 0).then_some(thread_root),
                saved_peer: match row.get::<_, i64>(2)? {
                    0 => None,
                    peer => Some(peer),
                },
                message_id: row.get(3)?,
            })
        })?
        .collect()
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
            "SELECT chat_id, kind, title, preview, status, unread_count, pinned, \
             can_pin_messages, has_topics, has_direct_messages FROM chats ORDER BY position IS \
             NULL, position, pinned DESC, chat_id DESC",
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
                has_topics: row.get(8)?,
                has_direct_messages: row.get(9)?,
                folders: Vec::new(),
            })
        })
        .context(LoadCacheSnafu)?
        .collect::<std::result::Result<_, _>>()
        .context(LoadCacheSnafu)
}

pub(super) fn load_topics(connection: &Connection) -> Result<Vec<StoredTopic>> {
    let mut statement = connection
        .prepare(
            "SELECT chat_id, topic_id, title, preview, timestamp, unread_count, pinned, closed, \
             hidden, icon_color, icon_emoji_id, top_message_id, draft_text, \
             draft_reply_to_message_id FROM topics ORDER BY chat_id, position",
        )
        .context(LoadCacheSnafu)?;
    statement
        .query_map([], |row| {
            Ok(StoredTopic {
                chat_id: row.get(0)?,
                id: row.get(1)?,
                title: row.get(2)?,
                preview: row.get(3)?,
                timestamp: row.get(4)?,
                unread: u32::try_from(row.get::<_, i64>(5)?).unwrap_or(0),
                pinned: row.get(6)?,
                closed: row.get(7)?,
                hidden: row.get(8)?,
                icon_color: u32::try_from(row.get::<_, i64>(9)?).unwrap_or(0),
                icon_emoji_id: row.get(10)?,
                top_message_id: row.get(11)?,
                draft_text: row.get(12)?,
                draft_reply_to: row.get(13)?,
            })
        })
        .context(LoadCacheSnafu)?
        .collect::<std::result::Result<_, _>>()
        .context(LoadCacheSnafu)
}

pub(super) fn load_saved_dialogs(connection: &Connection) -> Result<Vec<StoredSavedDialog>> {
    let mut statement = connection
        .prepare(
            "SELECT chat_id, saved_peer_id, title, preview, timestamp, pinned, top_message_id, \
             unread_count, unread_mark, draft_text, draft_reply_to_message_id FROM saved_dialogs \
             ORDER BY chat_id, position",
        )
        .context(LoadCacheSnafu)?;
    statement
        .query_map([], |row| {
            Ok(StoredSavedDialog {
                chat_id: row.get(0)?,
                peer_id: row.get(1)?,
                title: row.get(2)?,
                preview: row.get(3)?,
                timestamp: row.get(4)?,
                pinned: row.get(5)?,
                top_message_id: row.get(6)?,
                unread: u32::try_from(row.get::<_, i64>(7)?).unwrap_or(0),
                unread_mark: row.get(8)?,
                draft_text: row.get(9)?,
                draft_reply_to: row.get(10)?,
            })
        })
        .context(LoadCacheSnafu)?
        .collect::<std::result::Result<_, _>>()
        .context(LoadCacheSnafu)
}

pub(super) fn load_messages(connection: &Connection) -> Result<Vec<StoredMessage>> {
    query_messages(connection).context(LoadCacheSnafu)
}

pub(super) fn query_messages(connection: &Connection) -> rusqlite::Result<Vec<StoredMessage>> {
    let mut statement = connection.prepare(
        "SELECT chat_id, message_id, sender, body, timestamp, direction, delivery, \
         reply_to_message_id, thread_root_message_id, saved_peer_id, content_kind, metadata FROM \
         messages m WHERE NOT EXISTS (SELECT 1 FROM message_projections p WHERE p.chat_id = \
         m.chat_id AND p.message_id = m.message_id) ORDER BY m.chat_id, m.message_id",
    )?;
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
                saved_peer: row.get(9)?,
                content_kind: row.get(10)?,
                metadata: row.get(11)?,
            })
        })?
        .collect()
}

pub(super) fn load_pinned_messages(connection: &Connection) -> Result<Vec<StoredMessage>> {
    let mut statement = connection
        .prepare(
            "SELECT m.chat_id, m.message_id, m.sender, m.body, m.timestamp, m.direction, \
             m.delivery, m.reply_to_message_id, m.thread_root_message_id, m.saved_peer_id, \
             m.content_kind, m.metadata FROM messages m JOIN pinned_message_projection p ON \
             p.chat_id = m.chat_id AND p.message_id = m.message_id ORDER BY m.chat_id, \
             m.message_id",
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
                saved_peer: row.get(9)?,
                content_kind: row.get(10)?,
                metadata: row.get(11)?,
            })
        })
        .context(LoadCacheSnafu)?
        .collect::<std::result::Result<_, _>>()
        .context(LoadCacheSnafu)
}

pub(super) fn load_drafts(connection: &Connection) -> Result<Vec<StoredDraft>> {
    let mut statement = connection
        .prepare(
            "SELECT chat_id, thread_root_message_id, saved_peer_id, text, reply_to_message_id, \
             modified_at FROM drafts ORDER BY chat_id, thread_root_message_id, saved_peer_id",
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
                saved_peer: match row.get::<_, i64>(2)? {
                    0 => None,
                    peer => Some(peer),
                },
                text: row.get(3)?,
                reply_to: row.get(4)?,
                modified_at: row.get(5)?,
            })
        })
        .context(LoadCacheSnafu)?
        .collect::<std::result::Result<_, _>>()
        .context(LoadCacheSnafu)
}
use super::*;
