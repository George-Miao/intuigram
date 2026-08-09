use super::*;

pub(super) fn save_topics(
    connection: &Connection,
    chat_id: i64,
    topics: Vec<StoredTopic>,
) -> Result<()> {
    // Topic order is a complete Telegram projection, so replacement is safer
    // than attempting to infer deletions from incremental rows.
    let transaction = connection
        .unchecked_transaction()
        .context(SaveTopicsSnafu { chat_id })?;
    transaction
        .execute("DELETE FROM topics WHERE chat_id = ?1", [chat_id])
        .context(SaveTopicsSnafu { chat_id })?;
    for (position, topic) in topics.into_iter().enumerate() {
        let position = i64::try_from(position)
            .expect("an in-memory Topic list cannot exceed SQLite's signed index range");
        transaction
            .execute(
                "INSERT INTO topics(chat_id, topic_id, title, preview, timestamp, unread_count, \
                 pinned, closed, hidden, icon_color, icon_emoji_id, top_message_id, draft_text, \
                 draft_reply_to_message_id, position) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, \
                 ?10, ?11, ?12, ?13, ?14, ?15)",
                params![
                    chat_id,
                    topic.id,
                    topic.title,
                    topic.preview,
                    topic.timestamp,
                    topic.unread,
                    topic.pinned,
                    topic.closed,
                    topic.hidden,
                    topic.icon_color,
                    topic.icon_emoji_id,
                    topic.top_message_id,
                    topic.draft_text,
                    topic.draft_reply_to,
                    position,
                ],
            )
            .context(SaveTopicsSnafu { chat_id })?;
    }
    transaction.commit().context(SaveTopicsSnafu { chat_id })
}
