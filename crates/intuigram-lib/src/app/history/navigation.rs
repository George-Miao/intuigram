impl App {
    pub(in crate::app) fn move_chat(&mut self, forward: bool) -> Option<Effect> {
        if self.view.focus != Focus::Chats {
            return None;
        }
        let next = move_index(self.view.active_chat, self.view.chats.len(), forward);
        if next == self.view.active_chat {
            return None;
        }
        self.save_active_draft();
        self.save_transcript_anchor();
        self.clear_message_selection();
        self.view.active_thread = None;
        self.view.active_topic = None;
        self.view.active_saved_peer = None;
        self.view.chat_scroll_direction = if forward {
            ScrollDirection::Down
        } else {
            ScrollDirection::Up
        };
        self.view.active_chat = next;
        self.restore_active_topics();
        self.restore_active_saved_dialogs();
        self.restore_active_draft();
        let transcript_anchor = self
            .active_history_key()
            .and_then(|key| self.transcript_anchors.get(&key).copied());
        self.view.active_message = None;
        self.view.transcript_anchor = None;
        self.refresh_active_history_at(None, transcript_anchor);
        self.view.has_newer_messages = false;
        self.queue_active_media_previews();
        self.queue_visible_avatars();
        self.active_chat_id()
            .and_then(|chat| self.request_chat_load(chat))
            .or_else(|| Some(self.selection_effect()))
    }

    pub(in crate::app) fn target_previous_message(&mut self) {
        if self.view.messages.is_empty() {
            return;
        }
        if self.view.focus == Focus::Composer {
            self.save_active_draft();
        }
        self.view.active_message = Some(
            match self.view.active_message.or(self.view.transcript_anchor) {
                Some(index) => index.saturating_sub(1),
                None => self.view.messages.len() - 1,
            },
        );
        self.view.transcript_anchor = self.view.active_message;
        self.view.focus = Focus::Transcript;
    }

    pub(in crate::app) fn open_thread(&mut self) -> Option<Effect> {
        let chat = self.active_chat_id()?;
        let root = self.active_message_id()?;
        self.save_active_draft();
        self.save_transcript_anchor();
        self.clear_message_selection();
        self.view.active_thread = Some(root);
        self.view.active_message = None;
        self.view.transcript_anchor = None;
        self.restore_active_draft();
        self.refresh_active_history();
        self.view.focus = Focus::Composer;
        self.request_history_load(HistoryKey::scoped(
            chat,
            Some(root),
            self.view.active_saved_peer,
        ))
    }

    pub(in crate::app) fn leave_thread(&mut self) {
        self.save_active_draft();
        self.save_transcript_anchor();
        self.clear_message_selection();
        self.view.active_thread = None;
        self.view.active_message = None;
        self.view.transcript_anchor = None;
        self.restore_active_draft();
        let anchor = self
            .active_history_key()
            .and_then(|key| self.transcript_anchors.get(&key).copied());
        self.refresh_active_history_at(None, anchor);
        self.view.focus = Focus::Composer;
    }

    pub(in crate::app) fn target_next_message(&mut self) {
        if self.view.focus != Focus::Transcript {
            return;
        }
        let Some(index) = self.view.active_message else {
            self.view.focus = Focus::Composer;
            return;
        };
        if index + 1 < self.view.messages.len() {
            self.view.active_message = Some(index + 1);
            self.view.transcript_anchor = self.view.active_message;
        } else {
            if self.saved_history_is_read_only() {
                return;
            }
            self.focus_composer_at_anchor();
            self.view.has_newer_messages = false;
        }
    }

    pub(in crate::app) fn save_active_draft(&mut self) {
        if self.view.composer.editing.is_none()
            && !self.view.poll_composer
            && let Some(key) = self.active_history_key()
        {
            self.drafts.insert(key, self.view.composer.clone());
        }
    }

    pub(in crate::app) fn focus_composer_at_anchor(&mut self) {
        self.restore_recent_history_from_pin_projection();
        let reset_anchor = !self.view.selected_messages.is_empty();
        self.clear_message_selection();
        if reset_anchor {
            self.view.transcript_anchor = None;
        } else if self.view.active_message.is_some() {
            self.view.transcript_anchor = self.view.active_message;
        }
        self.view.active_message = None;
        self.view.focus = Focus::Composer;
    }

    pub(in crate::app) fn restore_active_draft(&mut self) {
        self.view.composer = self
            .active_history_key()
            .and_then(|key| self.drafts.get(&key).cloned())
            .unwrap_or_default();
    }

    pub(in crate::app) fn save_transcript_anchor(&mut self) {
        let Some(key) = self.active_history_key() else {
            return;
        };
        if let Some(anchor) = self.transcript_anchor_id() {
            self.transcript_anchors.insert(key, anchor);
        } else {
            self.transcript_anchors.remove(&key);
        }
    }

    pub(in crate::app) fn refresh_active_history(&mut self) {
        let active_message = self.active_message_id();
        let transcript_anchor = self.transcript_anchor_id();
        self.refresh_active_history_at(active_message, transcript_anchor);
    }

    pub(in crate::app) fn refresh_active_history_at(
        &mut self,
        active_message: Option<MessageId>,
        transcript_anchor: Option<MessageId>,
    ) {
        self.projected_pin = false;
        self.view.messages = self
            .active_history_key()
            .and_then(|key| self.histories.get(&key).cloned())
            .unwrap_or_default();
        self.view.parent_messages = if self.view.active_thread.is_some()
            && self.view.active_topic.is_none()
        {
            self.active_chat_id()
                .and_then(|chat| {
                    self.histories
                        .get(&HistoryKey::scoped(chat, None, self.view.active_saved_peer))
                })
                .cloned()
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        self.view.unread_boundary = self
            .active_history_key()
            .filter(|key| key.thread.is_none())
            .and_then(|key| self.unread_boundaries.get(&key).copied());
        self.refresh_pinned_projection();
        self.view.active_message =
            active_message.and_then(|message| self.history_position(message));
        self.view.transcript_anchor =
            transcript_anchor.and_then(|message| self.history_position(message));
        self.view.selected_messages.retain(|selected| {
            self.view
                .messages
                .iter()
                .any(|message| message.id == *selected)
        });
    }

    pub(in crate::app) fn history_position(&self, message: MessageId) -> Option<usize> {
        self.view
            .messages
            .iter()
            .position(|candidate| candidate.id == message)
    }

    pub(in crate::app) fn active_message_id(&self) -> Option<MessageId> {
        self.view
            .active_message
            .and_then(|index| self.view.messages.get(index))
            .map(|message| message.id)
    }

    pub(in crate::app) fn transcript_anchor_id(&self) -> Option<MessageId> {
        self.view
            .active_message
            .or(self.view.transcript_anchor)
            .and_then(|index| self.view.messages.get(index))
            .map(|message| message.id)
    }

    pub(in crate::app) fn active_chat_id(&self) -> Option<ChatId> {
        self.view
            .active_chat
            .and_then(|index| self.view.chats.get(index))
            .map(|chat| chat.id)
    }

    pub(in crate::app) fn active_history_key(&self) -> Option<HistoryKey> {
        self.active_chat_id().map(|chat| {
            HistoryKey::scoped(chat, self.view.active_thread, self.view.active_saved_peer)
        })
    }
}
use super::*;
