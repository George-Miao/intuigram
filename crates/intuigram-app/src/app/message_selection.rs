use super::*;

impl App {
    pub(super) fn toggle_message_selection(&mut self) {
        let Some(message) = self.active_message_id().filter(|message| message.0 > 0) else {
            return;
        };
        if let Some(index) = self
            .view
            .selected_messages
            .iter()
            .position(|selected| *selected == message)
        {
            self.view.selected_messages.remove(index);
            return;
        }
        self.view.selected_messages.push(message);
        self.view
            .selected_messages
            .sort_unstable_by_key(|selected| {
                self.view
                    .messages
                    .iter()
                    .position(|message| message.id == *selected)
                    .unwrap_or(usize::MAX)
            });
    }

    pub(super) fn selected_message_ids(&self) -> Vec<MessageId> {
        if self.view.selected_messages.is_empty() {
            return self
                .active_message_id()
                .filter(|message| message.0 > 0)
                .into_iter()
                .collect();
        }
        self.view
            .messages
            .iter()
            .filter(|message| message.id.0 > 0 && self.view.selected_messages.contains(&message.id))
            .map(|message| message.id)
            .collect()
    }

    pub(super) fn clear_message_selection(&mut self) {
        self.view.selected_messages.clear();
    }
}
