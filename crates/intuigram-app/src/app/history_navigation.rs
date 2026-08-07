#[derive(Default)]
pub(super) struct HistoryLoads {
    active: Option<HistoryKey>,
    queued: Option<HistoryKey>,
    background_cursor: usize,
    background_remaining: usize,
    refreshed_chats: HashSet<ChatId>,
    thread_read_pending: bool,
}

impl App {
    pub(super) fn history_load_is_active(&self) -> bool {
        self.history_loads.active.is_some()
    }

    pub(super) fn defer_active_thread_read(&mut self) {
        self.history_loads.thread_read_pending |= self.active_thread_read_effect().is_some();
    }

    pub(super) fn move_chat(&mut self, forward: bool) -> Option<Effect> {
        if self.view.focus != Focus::Chats {
            return None;
        }
        let next = move_index(self.view.active_chat, self.view.chats.len(), forward);
        if next == self.view.active_chat {
            return None;
        }
        self.save_active_draft();
        self.save_transcript_anchor();
        self.view.active_thread = None;
        self.view.active_chat = next;
        self.restore_active_draft();
        let transcript_anchor = self
            .active_history_key()
            .and_then(|key| self.transcript_anchors.get(&key).copied());
        self.view.active_message = None;
        self.view.transcript_anchor = None;
        self.refresh_active_history_at(None, transcript_anchor);
        self.view.has_newer_messages = false;
        self.queue_active_media_previews();
        self.active_chat_id()
            .and_then(|chat| self.request_chat_load(chat))
            .or_else(|| self.request_next_media_preview())
    }

    pub(super) fn request_chat_load(&mut self, chat: ChatId) -> Option<Effect> {
        self.request_history_load(HistoryKey { chat, thread: None })
    }

    pub(super) fn force_chat_load(&mut self, chat: ChatId) -> Option<Effect> {
        let key = HistoryKey { chat, thread: None };
        self.history_loads.refreshed_chats.remove(&chat);
        if self.history_loads.active == Some(key) {
            self.history_loads.queued = Some(key);
            None
        } else {
            self.start_history_load(key)
        }
    }

    pub(super) fn request_history_load(&mut self, key: HistoryKey) -> Option<Effect> {
        if self.history_was_refreshed(key) {
            return None;
        }
        self.start_history_load(key)
    }

    pub(super) fn reset_background_history(&mut self) {
        self.history_loads.active = None;
        self.history_loads.queued = None;
        self.history_loads.refreshed_chats.clear();
        self.history_loads.thread_read_pending = false;
        let active = self.active_chat_id();
        self.history_loads.background_cursor = active
            .and_then(|active| {
                self.all_chats
                    .iter()
                    .position(|chat| chat.id == active)
                    .map(|index| (index + 1) % self.all_chats.len())
            })
            .unwrap_or(0);
        self.history_loads.background_remaining = self
            .all_chats
            .len()
            .saturating_sub(usize::from(active.is_some()));
    }

    fn start_history_load(&mut self, key: HistoryKey) -> Option<Effect> {
        match self.history_loads.active {
            None => {
                self.history_loads.active = Some(key);
                Some(match key.thread {
                    Some(root) => Effect::LoadThread {
                        chat: key.chat,
                        root,
                    },
                    None => Effect::LoadChat { chat: key.chat },
                })
            }
            Some(loading) if loading == key => {
                self.history_loads.queued = None;
                None
            }
            Some(_) => {
                self.history_loads.queued = Some(key);
                None
            }
        }
    }

    pub(super) fn complete_history_load(
        &mut self,
        key: HistoryKey,
        refreshed: bool,
    ) -> Option<Effect> {
        if refreshed && key.thread.is_none() {
            self.history_loads.refreshed_chats.insert(key.chat);
        }
        if self.history_loads.active != Some(key) {
            return None;
        }
        self.history_loads.active = None;
        let foreground = self
            .history_loads
            .queued
            .take()
            .filter(|queued| !self.history_was_refreshed(*queued))
            .and_then(|queued| self.start_history_load(queued));
        foreground
            .or_else(|| self.request_next_media_preview())
            .or_else(|| self.request_next_background_history())
            .or_else(|| {
                self.history_loads
                    .thread_read_pending
                    .then(|| {
                        self.history_loads.thread_read_pending = false;
                        self.active_thread_read_effect()
                    })
                    .flatten()
            })
    }

    pub(super) fn request_next_background_history(&mut self) -> Option<Effect> {
        while self.history_loads.background_remaining > 0 {
            let index = self.history_loads.background_cursor;
            self.history_loads.background_cursor = (index + 1) % self.all_chats.len();
            self.history_loads.background_remaining -= 1;
            let key = HistoryKey {
                chat: self.all_chats[index].id,
                thread: None,
            };
            if !self.history_was_refreshed(key) {
                return self.start_history_load(key);
            }
        }
        None
    }

    fn history_was_refreshed(&self, key: HistoryKey) -> bool {
        key.thread.is_none() && self.history_loads.refreshed_chats.contains(&key.chat)
    }

    pub(super) fn move_folder(&mut self, forward: bool) {
        if self.view.focus == Focus::Chats {
            let active_chat = self.active_chat_id();
            self.save_active_draft();
            self.save_transcript_anchor();
            self.view.active_folder = move_index(
                Some(self.view.active_folder),
                self.view.folders.len(),
                forward,
            )
            .unwrap_or(0);
            self.refresh_folder_chats(active_chat);
            self.restore_active_draft();
            self.view.active_thread = None;
            let transcript_anchor = self
                .active_history_key()
                .and_then(|key| self.transcript_anchors.get(&key).copied());
            self.view.active_message = None;
            self.view.transcript_anchor = None;
            self.refresh_active_history_at(None, transcript_anchor);
        }
    }

    pub(super) fn refresh_folder_chats(&mut self, preferred: Option<ChatId>) {
        let folder = self
            .view
            .folders
            .get(self.view.active_folder)
            .map_or(0, |folder| folder.id);
        self.view.chats = self
            .all_chats
            .iter()
            .filter(|chat| chat.folders.contains(&folder))
            .cloned()
            .collect();
        self.view.active_chat = preferred
            .and_then(|chat| {
                self.view
                    .chats
                    .iter()
                    .position(|candidate| candidate.id == chat)
            })
            .or_else(|| (!self.view.chats.is_empty()).then_some(0));
    }

    pub(super) fn target_previous_message(&mut self) {
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

    pub(super) fn open_thread(&mut self) -> Option<Effect> {
        let chat = self.active_chat_id()?;
        let root = self.active_message_id()?;
        self.save_active_draft();
        self.save_transcript_anchor();
        self.view.active_thread = Some(root);
        self.view.active_message = None;
        self.view.transcript_anchor = None;
        self.restore_active_draft();
        self.refresh_active_history();
        self.view.focus = Focus::Composer;
        self.request_history_load(HistoryKey {
            chat,
            thread: Some(root),
        })
    }

    pub(super) fn leave_thread(&mut self) {
        self.save_active_draft();
        self.save_transcript_anchor();
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

    pub(super) fn target_next_message(&mut self) {
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
            self.focus_composer_at_anchor();
            self.view.has_newer_messages = false;
        }
    }

    pub(super) fn save_active_draft(&mut self) {
        if self.view.composer.editing.is_none()
            && !self.view.poll_composer
            && let Some(key) = self.active_history_key()
        {
            self.drafts.insert(key, self.view.composer.clone());
        }
    }

    pub(super) fn focus_composer_at_anchor(&mut self) {
        self.restore_recent_history_from_pin_projection();
        if self.view.active_message.is_some() {
            self.view.transcript_anchor = self.view.active_message;
        }
        self.view.active_message = None;
        self.view.focus = Focus::Composer;
    }

    pub(super) fn restore_active_draft(&mut self) {
        self.view.composer = self
            .active_history_key()
            .and_then(|key| self.drafts.get(&key).cloned())
            .unwrap_or_default();
    }

    pub(super) fn save_transcript_anchor(&mut self) {
        let Some(key) = self.active_history_key() else {
            return;
        };
        if let Some(anchor) = self.transcript_anchor_id() {
            self.transcript_anchors.insert(key, anchor);
        } else {
            self.transcript_anchors.remove(&key);
        }
    }

    pub(super) fn refresh_active_history(&mut self) {
        let active_message = self.active_message_id();
        let transcript_anchor = self.transcript_anchor_id();
        self.refresh_active_history_at(active_message, transcript_anchor);
    }

    pub(super) fn refresh_active_history_at(
        &mut self,
        active_message: Option<MessageId>,
        transcript_anchor: Option<MessageId>,
    ) {
        self.projected_pin = false;
        self.view.messages = self
            .active_history_key()
            .and_then(|key| self.histories.get(&key).cloned())
            .unwrap_or_default();
        self.view.unread_boundary = self
            .active_history_key()
            .filter(|key| key.thread.is_none())
            .and_then(|key| self.unread_boundaries.get(&key).copied());
        self.refresh_pinned_projection();
        self.view.active_message =
            active_message.and_then(|message| self.history_position(message));
        self.view.transcript_anchor =
            transcript_anchor.and_then(|message| self.history_position(message));
    }

    pub(super) fn history_position(&self, message: MessageId) -> Option<usize> {
        self.view
            .messages
            .iter()
            .position(|candidate| candidate.id == message)
    }

    pub(super) fn active_message_id(&self) -> Option<MessageId> {
        self.view
            .active_message
            .and_then(|index| self.view.messages.get(index))
            .map(|message| message.id)
    }

    pub(super) fn transcript_anchor_id(&self) -> Option<MessageId> {
        self.view
            .active_message
            .or(self.view.transcript_anchor)
            .and_then(|index| self.view.messages.get(index))
            .map(|message| message.id)
    }

    pub(super) fn active_chat_id(&self) -> Option<ChatId> {
        self.view
            .active_chat
            .and_then(|index| self.view.chats.get(index))
            .map(|chat| chat.id)
    }

    pub(super) fn active_history_key(&self) -> Option<HistoryKey> {
        self.active_chat_id().map(|chat| HistoryKey {
            chat,
            thread: self.view.active_thread,
        })
    }

    pub(super) fn at_latest(&self) -> bool {
        self.view
            .active_message
            .or(self.view.transcript_anchor)
            .is_none_or(|index| Some(index) == self.view.messages.len().checked_sub(1))
    }

    pub(super) fn active_thread_read_effect(&self) -> Option<Effect> {
        let key = self.active_history_key()?;
        let root = key.thread?;
        if self.view.focus == Focus::Chats || !self.at_latest() {
            return None;
        }
        let max_id = self
            .view
            .messages
            .iter()
            .filter(|message| message.direction == MessageDirection::Incoming && message.id.0 > 0)
            .map(|message| message.id)
            .max()?;
        Some(Effect::ReadThread {
            chat: key.chat,
            root,
            max_id,
        })
    }
}
use super::*;
