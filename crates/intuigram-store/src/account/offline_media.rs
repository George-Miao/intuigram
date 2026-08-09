use super::*;

pub(super) fn set_chat_media_offline(
    connection: &Connection,
    chat_id: i64,
    keep: bool,
) -> Result<()> {
    if keep {
        connection
            .execute(
                "INSERT OR IGNORE INTO offline_media_chats(chat_id) VALUES (?1)",
                [chat_id],
            )
            .context(SaveOfflineMediaPolicySnafu { chat_id })?;
    } else {
        connection
            .execute(
                "DELETE FROM offline_media_chats WHERE chat_id = ?1",
                [chat_id],
            )
            .context(SaveOfflineMediaPolicySnafu { chat_id })?;
    }
    Ok(())
}

pub(super) fn load_offline_chats(connection: &Connection) -> Result<Vec<i64>> {
    let mut statement = connection
        .prepare("SELECT chat_id FROM offline_media_chats ORDER BY chat_id")
        .context(LoadCacheSnafu)?;
    statement
        .query_map([], |row| row.get(0))
        .context(LoadCacheSnafu)?
        .collect::<std::result::Result<_, _>>()
        .context(LoadCacheSnafu)
}
