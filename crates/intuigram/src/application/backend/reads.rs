use super::*;

impl Backend {
    pub(super) async fn execute_thread_read(
        &mut self,
        chat: ChatId,
        root: MessageId,
        max_id: MessageId,
    ) -> Result<Option<AdapterEvent>> {
        match self.client.read_thread(chat, root, max_id).await {
            Ok(()) => Ok(None),
            Err(source) if source.is_connection_failure() => Err(Error::Telegram { source }),
            Err(error) => Ok(Some(AdapterEvent::OperationFailed(error.to_string()))),
        }
    }

    pub(super) async fn execute_history_read(
        &mut self,
        chat: ChatId,
        saved_peer: Option<ChatId>,
        max_id: MessageId,
    ) -> Result<Option<AdapterEvent>> {
        let result = match saved_peer {
            Some(peer) => self.client.read_saved_history(chat, peer, max_id).await,
            None => self.client.read_history(chat, max_id).await,
        };
        match result {
            Ok(()) => Ok(Some(AdapterEvent::HistoryRead {
                chat,
                saved_peer,
                max_id,
                outgoing: false,
                unread: Some(0),
            })),
            Err(source) if source.is_connection_failure() => Err(Error::Telegram { source }),
            Err(error) => Ok(Some(AdapterEvent::OperationFailed(error.to_string()))),
        }
    }
}
