use super::*;

impl App {
    pub(super) fn reconcile_message_pins(
        &mut self,
        chat: ChatId,
        ids: &[MessageId],
        pinned: bool,
    ) -> Option<Effect> {
        let missing = self.apply_message_pins(chat, ids, pinned);
        (pinned && missing)
            .then(|| self.force_chat_load(chat))
            .flatten()
    }

    pub(super) fn refresh_pinned_projection(&mut self) {
        self.view.pinned_messages = self
            .active_chat_id()
            .and_then(|chat| self.pinned_histories.get(&chat).cloned())
            .unwrap_or_default();
    }

    pub(super) fn store_loaded_pins(&mut self, chat: ChatId, mut messages: Vec<MessageView>) {
        messages.sort_by_key(|message| message.id);
        self.pinned_histories.insert(chat, messages);
        if self.active_chat_id() == Some(chat) {
            self.refresh_pinned_projection();
        }
    }

    pub(super) fn apply_chat_pin_permission(
        &mut self,
        chat: ChatId,
        can_pin_messages: bool,
    ) -> Option<Effect> {
        for candidate in self
            .all_chats
            .iter_mut()
            .chain(self.view.chats.iter_mut())
            .filter(|candidate| candidate.id == chat)
        {
            candidate.can_pin_messages = can_pin_messages;
        }
        None
    }

    pub(super) fn navigate_pinned(&mut self) {
        if self.view.active_thread.is_some() {
            return;
        }
        let current = self.active_message_id().filter(|id| {
            self.view
                .pinned_messages
                .iter()
                .any(|message| message.id == *id)
        });
        let target = self
            .view
            .pinned_messages
            .iter()
            .rev()
            .map(|message| message.id)
            .find(|id| current.is_none_or(|current| *id < current))
            .or_else(|| self.view.pinned_messages.last().map(|message| message.id));
        if let Some(target) = target {
            let projected = self.is_showing_pin_projection();
            let target_is_recent = self
                .active_history_key()
                .and_then(|key| self.histories.get(&key))
                .is_some_and(|history| history.iter().any(|message| message.id == target));
            self.save_active_draft();
            if target_is_recent {
                self.refresh_active_history_at(Some(target), Some(target));
            } else if let Some(message) = self
                .view
                .pinned_messages
                .iter()
                .find(|message| message.id == target)
                .cloned()
            {
                if !projected {
                    self.save_transcript_anchor();
                }
                self.view.messages = vec![message];
                self.view.active_message = Some(0);
                self.view.transcript_anchor = Some(0);
                self.projected_pin = true;
            }
            self.view.focus = Focus::Transcript;
        }
    }

    pub(super) fn restore_recent_history_from_pin_projection(&mut self) {
        if !self.projected_pin {
            return;
        }
        let anchor = self
            .active_history_key()
            .and_then(|key| self.transcript_anchors.get(&key).copied());
        self.refresh_active_history_at(None, anchor);
    }

    fn is_showing_pin_projection(&self) -> bool {
        self.projected_pin
    }

    pub(super) fn toggle_active_pin(&self) -> Option<Effect> {
        let chat = self.active_chat_id()?;
        let message = self
            .view
            .active_message
            .and_then(|index| self.view.messages.get(index))?;
        if message.id.0 <= 0 {
            return None;
        }
        let pinned = !message.details.pinned;
        Some(Effect::SetMessagePinned {
            chat,
            message: message.id,
            pinned,
        })
    }
}
