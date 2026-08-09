use intuigram::encode_stored_message;
use intuigram_app::{AdapterEvent, ChatId, SavedDialogListView};
use intuigram_store::StoredSavedDialog;
use snafu::ResultExt;

use super::TestSystem;
use super::telegram_control::block_on;
use crate::error::{Result, StoreSnafu};

impl TestSystem {
    pub(super) fn handle_saved_dialog_load(&mut self, chat: ChatId) -> Result<()> {
        let dialogs = self
            .telegram
            .load_saved_dialogs(chat)
            .map_err(|error| self.scenario_error(error))?;
        self.database
            .save_saved_dialogs(
                chat.0,
                dialogs
                    .iter()
                    .map(|dialog| StoredSavedDialog {
                        chat_id: chat.0,
                        peer_id: dialog.peer.0,
                        title: dialog.title.clone(),
                        preview: dialog.preview.clone(),
                        timestamp: dialog.timestamp.clone(),
                        pinned: dialog.pinned,
                        top_message_id: dialog.top_message.0,
                    })
                    .collect(),
            )
            .context(StoreSnafu)?;
        self.application
            .handle_adapter(AdapterEvent::SavedDialogsLoaded(SavedDialogListView {
                chat,
                dialogs,
            }));
        Ok(())
    }

    pub(super) fn handle_saved_history_load(&mut self, chat: ChatId, peer: ChatId) -> Result<()> {
        let messages = self
            .telegram
            .load_saved_history(chat, peer)
            .map_err(|error| self.scenario_error(error))?;
        let request = self
            .database
            .store()
            .save_messages(
                messages
                    .iter()
                    .map(|message| encode_stored_message(chat, message))
                    .collect(),
            )
            .context(StoreSnafu)?;
        block_on(request).context(StoreSnafu)?;
        self.application
            .handle_adapter(AdapterEvent::SavedHistoryLoaded {
                chat,
                peer,
                messages,
            });
        Ok(())
    }
}
