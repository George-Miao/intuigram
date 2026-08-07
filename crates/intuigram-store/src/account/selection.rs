use super::*;

pub(super) fn save_selection(connection: &Connection, selection: StoredSelection) -> Result<()> {
    connection
        .execute(
            "INSERT OR REPLACE INTO ui_selection (singleton, folder_id, chat_id) VALUES (1, ?1, \
             ?2)",
            params![selection.folder_id, selection.chat_id],
        )
        .map(|_| ())
        .context(SaveSelectionSnafu)
}
