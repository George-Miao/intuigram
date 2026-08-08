use super::*;

impl Backend {
    pub(in crate::application) async fn set_message_pinned(
        &mut self,
        chat: ChatId,
        message: MessageId,
        pinned: bool,
    ) -> Result<BackendOutput> {
        match self.client.set_message_pinned(chat, message, pinned).await {
            Ok(update) => Ok(BackendOutput::telegram_update(update)),
            Err(source) if source.is_connection_failure() => Err(Error::Telegram { source }),
            Err(error) => Ok(BackendOutput::event(Some(AdapterEvent::OperationFailed(
                error.to_string(),
            )))),
        }
    }
}
