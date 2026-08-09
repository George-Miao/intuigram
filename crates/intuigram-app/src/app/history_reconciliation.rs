use super::*;

impl App {
    pub(super) fn apply_message_pins(
        &mut self,
        chat: ChatId,
        ids: &[MessageId],
        pinned: bool,
    ) -> bool {
        let missing = ids.iter().any(|id| {
            !self
                .histories
                .get(&HistoryKey::root(chat))
                .is_some_and(|history| history.iter().any(|message| message.id == *id))
        });
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
        let pins = self.pinned_histories.entry(chat).or_default();
        if pinned {
            for id in ids {
                if pins.iter().all(|message| message.id != *id)
                    && let Some(message) = self
                        .histories
                        .get(&HistoryKey::root(chat))
                        .and_then(|history| history.iter().find(|message| message.id == *id))
                {
                    let mut message = message.clone();
                    message.details.pinned = true;
                    pins.push(message);
                }
            }
            pins.sort_by_key(|message| message.id);
        } else {
            pins.retain(|message| !ids.contains(&message.id));
        }
        if self.active_chat_id() == Some(chat) {
            for message in &mut self.view.messages {
                if ids.contains(&message.id) {
                    message.details.pinned = pinned;
                }
            }
            self.refresh_pinned_projection();
        }
        missing
    }

    pub(super) fn store_loaded_history(&mut self, key: HistoryKey, refreshed: Vec<MessageView>) {
        let messages = reconcile_refresh(
            self.histories.get(&key).map(Vec::as_slice),
            refreshed,
            self.history_request_baseline(key),
            if key.thread.is_some() {
                RefreshScope::Thread
            } else {
                RefreshScope::Root
            },
        );
        self.ensure_unread_boundary(key, &messages);
        if self.active_history_key() != Some(key) {
            self.histories.insert(key, messages);
            return;
        }
        if self.view.focus == Focus::Transcript && !self.view.messages.is_empty() {
            self.view.has_newer_messages |= self.view.messages != messages;
            self.histories.insert(key, messages);
            self.refresh_pinned_projection();
            return;
        }
        let active_message = self.active_message_id();
        let transcript_anchor = self.transcript_anchor_id();
        self.histories.insert(key, messages);
        self.refresh_active_history_at(active_message, transcript_anchor);
        self.view.has_newer_messages = false;
    }
}
