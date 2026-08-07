use super::*;

impl App {
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
