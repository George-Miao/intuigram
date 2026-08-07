use super::*;

pub(super) fn save_selection(connection: &Connection, selection: StoredSelection) -> Result<()> {
    connection
        .execute(
            "INSERT OR REPLACE INTO ui_selection (singleton, folder_id, chat_id, \
             anchor_message_id) VALUES (1, ?1, ?2, ?3)",
            params![
                selection.folder_id,
                selection.chat_id,
                selection.anchor_message_id
            ],
        )
        .map(|_| ())
        .context(SaveSelectionSnafu)
}
