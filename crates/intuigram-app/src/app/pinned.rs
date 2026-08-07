use super::*;

impl App {
    pub(super) fn toggle_active_pin(&self) -> Option<Effect> {
        let chat = self.active_chat_id()?;
        let mut message = self
            .view
            .active_message
            .and_then(|index| self.view.messages.get(index))?
            .clone();
        if message.id.0 <= 0 {
            return None;
        }
        let pinned = !message.details.pinned;
        message.details.pinned = pinned;
        Some(Effect::SetMessagePinned {
            chat,
            message: Box::new(message),
            pinned,
        })
    }
}
