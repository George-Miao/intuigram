use super::*;

impl Backend {
    pub(super) async fn set_message_pinned(
        &mut self,
        chat: ChatId,
        message: MessageView,
        pinned: bool,
    ) -> Result<Option<AdapterEvent>> {
        match self
            .client
            .set_message_pinned(chat, message.id, pinned)
            .await
        {
            Ok(()) => {
                self.store
                    .save_messages(vec![encode_stored_message(chat, &message)])
                    .context(AccountDatabaseSnafu)?
                    .await
                    .context(AccountDatabaseSnafu)?;
                Ok(Some(AdapterEvent::MessageUpdated {
                    chat,
                    message: Box::new(message),
                }))
            }
            Err(source) if source.is_connection_failure() => Err(Error::Telegram { source }),
            Err(error) => Ok(Some(AdapterEvent::OperationFailed(error.to_string()))),
        }
    }
}
