use super::*;

/// Store-owned Saved Messages origin or monoforum direct-message dialog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredSavedDialog {
    /// Owning Saved Messages Chat.
    pub chat_id: i64,

    /// Original peer used by Telegram's Saved Messages filter.
    pub peer_id: i64,

    /// Current peer display title.
    pub title: String,

    /// Latest saved Message fallback.
    pub preview: String,

    /// Latest saved Message timestamp.
    pub timestamp: String,

    /// Telegram pin state.
    pub pinned: bool,

    /// Dialog-local unread Message count.
    pub unread: u32,

    /// Whether Telegram manually marks the dialog unread.
    pub unread_mark: bool,

    /// Latest Message identity in the Saved Messages Chat.
    pub top_message_id: i64,

    /// Server Draft text for a writable monoforum dialog.
    pub draft_text: Option<String>,

    /// Server Draft reply target for a writable monoforum dialog.
    pub draft_reply_to: Option<i64>,
}

pub(super) fn save_saved_dialogs(
    connection: &Connection,
    chat_id: i64,
    dialogs: Vec<StoredSavedDialog>,
) -> Result<()> {
    // This handwritten SQL is necessary because saved dialogs are an ordered,
    // authoritative Telegram projection that the generic Message upsert cannot
    // represent or prune safely.
    let transaction = connection
        .unchecked_transaction()
        .context(SaveSavedDialogsSnafu { chat_id })?;
    transaction
        .execute("DELETE FROM saved_dialogs WHERE chat_id = ?1", [chat_id])
        .context(SaveSavedDialogsSnafu { chat_id })?;
    for (position, dialog) in dialogs.into_iter().enumerate() {
        let position = i64::try_from(position)
            .expect("an in-memory saved dialog list cannot exceed SQLite's signed index range");
        transaction
            .execute(
                "INSERT INTO saved_dialogs(chat_id, saved_peer_id, title, preview, timestamp, \
                 pinned, top_message_id, position, unread_count, unread_mark, draft_text, \
                 draft_reply_to_message_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, \
                 ?12)",
                params![
                    chat_id,
                    dialog.peer_id,
                    dialog.title,
                    dialog.preview,
                    dialog.timestamp,
                    dialog.pinned,
                    dialog.top_message_id,
                    position,
                    dialog.unread,
                    dialog.unread_mark,
                    dialog.draft_text,
                    dialog.draft_reply_to,
                ],
            )
            .context(SaveSavedDialogsSnafu { chat_id })?;
    }
    transaction
        .commit()
        .context(SaveSavedDialogsSnafu { chat_id })
}
