use super::*;

impl App {
    pub(super) fn apply_edit_acknowledgement(
        &mut self,
        chat: ChatId,
        message: MessageId,
        text: String,
        entities: Vec<TextEntity>,
    ) {
        for candidate in self
            .histories
            .iter_mut()
            .filter(|(key, _)| key.chat == chat)
            .flat_map(|(_, history)| history)
            .filter(|candidate| candidate.id == message)
        {
            candidate.body.clone_from(&text);
            candidate.details.entities.clone_from(&entities);
            candidate.details.edited = true;
        }
        if self.active_chat_id() == Some(chat) {
            self.refresh_active_history();
        }
    }

    pub(super) fn apply_message_media(
        &mut self,
        chat: ChatId,
        message: MessageId,
        media: MediaCard,
    ) {
        for candidate in self
            .histories
            .iter_mut()
            .filter(|(key, _)| key.chat == chat)
            .flat_map(|(_, history)| history)
            .filter(|candidate| candidate.id == message)
        {
            candidate.details.media = Some(media.clone());
        }
        if self.active_chat_id() == Some(chat) {
            self.refresh_active_history();
        }
    }
}
