use super::*;

impl App {
    pub(super) fn active_read_effect(&self) -> Option<Effect> {
        let key = self.active_history_key()?;
        if self.view.focus == Focus::Chats || !self.at_latest() {
            return None;
        }
        if key.thread.is_none() && self.chat_unread(key.chat) == 0 {
            return None;
        }
        let max_id = self
            .view
            .messages
            .iter()
            .filter(|message| message.direction == MessageDirection::Incoming && message.id.0 > 0)
            .map(|message| message.id)
            .max()?;
        Some(match key.thread {
            Some(root) => Effect::ReadThread {
                chat: key.chat,
                root,
                max_id,
            },
            None => Effect::ReadHistory {
                chat: key.chat,
                max_id,
            },
        })
    }

    pub(super) fn rebuild_unread_boundaries(&mut self) {
        self.unread_boundaries.clear();
        let boundaries = self
            .histories
            .iter()
            .filter(|(key, _)| key.thread.is_none())
            .filter_map(|(key, messages)| {
                unread_boundary(messages, self.chat_unread(key.chat)).map(|id| (*key, id))
            })
            .collect::<Vec<_>>();
        self.unread_boundaries.extend(boundaries);
    }

    pub(super) fn ensure_unread_boundary(&mut self, key: HistoryKey, messages: &[MessageView]) {
        if key.thread.is_some() || self.unread_boundaries.contains_key(&key) {
            return;
        }
        if let Some(boundary) = unread_boundary(messages, self.chat_unread(key.chat)) {
            self.unread_boundaries.insert(key, boundary);
        }
    }

    pub(super) fn advance_unread_boundary(&mut self, chat: ChatId, max_id: MessageId, unread: u32) {
        let key = HistoryKey { chat, thread: None };
        if unread == 0 {
            self.unread_boundaries.remove(&key);
            return;
        }
        let Some(messages) = self.histories.get(&key) else {
            self.unread_boundaries.remove(&key);
            return;
        };
        let boundary = messages
            .iter()
            .find(|message| {
                message.direction == MessageDirection::Incoming && message.id.0 > max_id.0
            })
            .map(|message| message.id)
            .or_else(|| unread_boundary(messages, unread));
        if let Some(boundary) = boundary {
            self.unread_boundaries.insert(key, boundary);
        } else {
            self.unread_boundaries.remove(&key);
        }
    }

    pub(super) fn chat_unread(&self, chat: ChatId) -> u32 {
        self.all_chats
            .iter()
            .find(|candidate| candidate.id == chat)
            .map_or(0, |candidate| candidate.unread)
    }
}

fn unread_boundary(messages: &[MessageView], unread: u32) -> Option<MessageId> {
    let unread = usize::try_from(unread).unwrap_or(usize::MAX);
    if unread == 0 {
        return None;
    }
    messages
        .iter()
        .rev()
        .filter(|message| message.direction == MessageDirection::Incoming)
        .nth(unread.saturating_sub(1))
        .or_else(|| {
            messages
                .iter()
                .find(|message| message.direction == MessageDirection::Incoming)
        })
        .map(|message| message.id)
}
