use super::*;

impl App {
    pub(super) fn apply_message_pins(&mut self, chat: ChatId, ids: &[MessageId], pinned: bool) {
        let active_message = self.active_message_id();
        let transcript_anchor = self.transcript_anchor_id();
        for (key, history) in &mut self.histories {
            if key.chat != chat {
                continue;
            }
            for message in history {
                if ids.contains(&message.id) {
                    message.details.pinned = pinned;
                }
            }
        }
        if self.active_chat_id() == Some(chat) {
            self.refresh_active_history_at(active_message, transcript_anchor);
        }
    }

    pub(super) fn store_loaded_history(&mut self, key: HistoryKey, refreshed: Vec<MessageView>) {
        let messages = reconcile_refresh(self.histories.get(&key).map(Vec::as_slice), refreshed);
        if self.active_history_key() != Some(key) {
            self.histories.insert(key, messages);
            return;
        }
        if self.view.focus == Focus::Transcript {
            self.view.has_newer_messages |= self.view.messages != messages;
            self.histories.insert(key, messages);
            return;
        }
        let active_message = self.active_message_id();
        let transcript_anchor = self.transcript_anchor_id();
        self.histories.insert(key, messages);
        self.refresh_active_history_at(active_message, transcript_anchor);
        self.view.has_newer_messages = false;
    }
}
