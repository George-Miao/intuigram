use super::*;

pub(super) fn save_selection(connection: &Connection, selection: StoredSelection) -> Result<()> {
    let transaction = connection
        .unchecked_transaction()
        .context(SaveSelectionSnafu)?;
    transaction
        .execute(
            "INSERT OR REPLACE INTO ui_selection (singleton, folder_id, chat_id, \
             anchor_message_id) VALUES (1, ?1, ?2, ?3)",
            params![
                selection.folder_id,
                selection.chat_id,
                selection.anchor_message_id
            ],
        )
        .context(SaveSelectionSnafu)?;
    transaction
        .execute("DELETE FROM transcript_anchors", [])
        .context(SaveSelectionSnafu)?;
    for anchor in selection.transcript_anchors {
        transaction
            .execute(
                "INSERT INTO transcript_anchors(chat_id, thread_root_message_id, saved_peer_id, \
                 anchor_message_id) VALUES (?1, ?2, ?3, ?4)",
                params![
                    anchor.chat_id,
                    anchor.thread_root.unwrap_or_default(),
                    anchor.saved_peer.unwrap_or_default(),
                    anchor.message_id
                ],
            )
            .context(SaveSelectionSnafu)?;
    }
    transaction.commit().context(SaveSelectionSnafu)
}
