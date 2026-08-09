use super::*;

impl Backend {
    pub(super) async fn load_saved_dialogs(
        &mut self,
        chat: ChatId,
    ) -> Result<Option<AdapterEvent>> {
        match self.client.saved_dialogs().await {
            Ok(dialogs) => {
                self.store
                    .save_saved_dialogs(
                        chat.0,
                        dialogs
                            .iter()
                            .map(|dialog| stored_saved_dialog(chat, dialog))
                            .collect(),
                    )
                    .context(AccountDatabaseSnafu)?
                    .await
                    .context(AccountDatabaseSnafu)?;
                Ok(Some(AdapterEvent::SavedDialogsLoaded(
                    SavedDialogListView { chat, dialogs },
                )))
            }
            Err(source) if source.is_connection_failure() => Err(Error::Telegram { source }),
            Err(error) => Ok(Some(AdapterEvent::SavedDialogsLoadFailed(
                SavedDialogLoadFailure {
                    chat,
                    reason: error.to_string(),
                },
            ))),
        }
    }

    pub(super) async fn load_saved_history(
        &mut self,
        chat: ChatId,
        peer: ChatId,
    ) -> Result<Option<AdapterEvent>> {
        match self.client.saved_history(peer, 100).await {
            Ok(messages) => {
                self.store
                    .save_messages(
                        messages
                            .iter()
                            .map(|message| encode_stored_message(chat, message))
                            .collect(),
                    )
                    .context(AccountDatabaseSnafu)?
                    .await
                    .context(AccountDatabaseSnafu)?;
                Ok(Some(AdapterEvent::SavedHistoryLoaded {
                    chat,
                    peer,
                    messages,
                }))
            }
            Err(source) if source.is_connection_failure() => Err(Error::Telegram { source }),
            Err(error) => Ok(Some(AdapterEvent::SavedHistoryLoadFailed {
                chat,
                peer,
                reason: error.to_string(),
            })),
        }
    }
}

fn stored_saved_dialog(chat: ChatId, dialog: &SavedDialogView) -> StoredSavedDialog {
    StoredSavedDialog {
        chat_id: chat.0,
        peer_id: dialog.peer.0,
        title: dialog.title.clone(),
        preview: dialog.preview.clone(),
        timestamp: dialog.timestamp.clone(),
        pinned: dialog.pinned,
        top_message_id: dialog.top_message.0,
    }
}
