use super::*;

const BACKGROUND_HISTORY_LIMIT: usize = 16;

#[derive(Default)]
pub(super) struct HistoryLoads {
    active: Option<HistoryKey>,
    active_baseline: HashSet<MessageId>,
    queued: Option<HistoryKey>,
    background_cursor: usize,
    background_remaining: usize,
    refreshed_chats: HashSet<ChatId>,
    read_pending: bool,
}

impl App {
    pub(super) fn history_load_is_active(&self) -> bool {
        self.history_loads.active.is_some()
    }

    pub(super) fn defer_active_read(&mut self) {
        self.history_loads.read_pending |= self.active_read_effect().is_some();
    }

    pub(super) fn take_pending_read(&mut self) -> Option<Effect> {
        if !self.history_loads.read_pending {
            return None;
        }
        self.history_loads.read_pending = false;
        self.active_read_effect()
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
        self.history_loads.active_baseline.clear();
        self.history_loads.queued = None;
        self.history_loads.refreshed_chats.clear();
        self.history_loads.read_pending = false;
        self.view.chat_loading = ChatLoadingState::Idle;
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
            .saturating_sub(usize::from(active.is_some()))
            .min(BACKGROUND_HISTORY_LIMIT);
    }

    pub(super) fn reset_reconnected_history(&mut self) {
        self.reset_background_history();
        if let Some(index) = self
            .active_chat_id()
            .and_then(|active| self.all_chats.iter().position(|chat| chat.id == active))
        {
            self.history_loads.background_cursor = index;
            self.history_loads.background_remaining =
                self.all_chats.len().min(BACKGROUND_HISTORY_LIMIT);
        }
    }

    fn start_history_load(&mut self, key: HistoryKey) -> Option<Effect> {
        if self.active_history_key() == Some(key) {
            self.view.chat_loading = if self.view.messages.is_empty() {
                ChatLoadingState::Fresh
            } else {
                ChatLoadingState::Updating
            };
        }
        match self.history_loads.active {
            None => {
                self.history_loads.active = Some(key);
                self.history_loads.active_baseline = self
                    .histories
                    .get(&key)
                    .into_iter()
                    .flatten()
                    .map(|message| message.id)
                    .collect();
                Some(match key.thread {
                    Some(root) => Effect::LoadThread {
                        chat: key.chat,
                        root,
                    },
                    None => Effect::LoadChat {
                        chat: key.chat,
                        selection: (self.active_history_key() == Some(key))
                            .then(|| self.selection_view()),
                        transcript_anchors: self.transcript_anchor_views(),
                    },
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

    pub(super) fn history_request_baseline(&self, key: HistoryKey) -> Option<&HashSet<MessageId>> {
        (self.history_loads.active == Some(key)).then_some(&self.history_loads.active_baseline)
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
        if self.active_history_key() == Some(key) {
            self.view.chat_loading = ChatLoadingState::Idle;
        }
        self.history_loads.active = None;
        self.history_loads.active_baseline.clear();
        let foreground = self
            .history_loads
            .queued
            .take()
            .filter(|queued| !self.history_was_refreshed(*queued))
            .and_then(|queued| self.start_history_load(queued));
        foreground
            .or_else(|| self.request_next_media_preview())
            .or_else(|| self.request_next_avatar())
            .or_else(|| self.request_next_background_history())
            .or_else(|| {
                self.history_loads
                    .read_pending
                    .then(|| {
                        self.history_loads.read_pending = false;
                        self.active_read_effect()
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
}
