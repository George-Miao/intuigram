use super::*;

impl App {
    pub(super) fn apply_chat_status(&mut self, chat: ChatId, status: String) {
        for chat_view in self
            .all_chats
            .iter_mut()
            .chain(self.view.chats.iter_mut())
            .filter(|candidate| candidate.id == chat)
        {
            chat_view.status.clone_from(&status);
        }
    }

    pub(super) fn apply_folder_membership(
        &mut self,
        chat: ChatId,
        folder: i32,
        included: bool,
    ) -> Option<Effect> {
        let active_chat = self.active_chat_id();
        self.save_active_draft();
        self.save_transcript_anchor();
        self.clear_message_selection();
        for chat_view in self
            .all_chats
            .iter_mut()
            .chain(self.view.chats.iter_mut())
            .filter(|candidate| candidate.id == chat)
        {
            chat_view.folders.retain(|candidate| *candidate != folder);
            if folder == -1 {
                chat_view.folders.retain(|candidate| *candidate != 0);
                chat_view.folders.push(if included { -1 } else { 0 });
            } else if included {
                chat_view.folders.push(folder);
            }
            chat_view.folders.sort_unstable();
            chat_view.folders.dedup();
        }
        self.refresh_folder_unread();
        self.refresh_folder_chats(active_chat);
        if self.active_chat_id() == active_chat {
            return None;
        }

        self.view.active_thread = None;
        self.view.active_message = None;
        self.view.transcript_anchor = None;
        self.restore_active_draft();
        let transcript_anchor = self
            .active_history_key()
            .and_then(|key| self.transcript_anchors.get(&key).copied());
        self.refresh_active_history_at(None, transcript_anchor);
        self.view.has_newer_messages = false;
        self.active_chat_id()
            .and_then(|chat| self.request_chat_load(chat))
    }

    pub(super) fn replace_message(&mut self, chat: ChatId, message: MessageView) {
        let active_message = (self.active_chat_id() == Some(chat))
            .then(|| self.active_message_id())
            .flatten();
        let transcript_anchor = (self.active_chat_id() == Some(chat))
            .then(|| self.transcript_anchor_id())
            .flatten();
        let root_key = HistoryKey { chat, thread: None };
        upsert_history_message(self.histories.entry(root_key).or_default(), &message);
        if let Some(root) = message.details.thread_root {
            let thread_key = HistoryKey {
                chat,
                thread: Some(root),
            };
            upsert_history_message(self.histories.entry(thread_key).or_default(), &message);
        }
        for (_, history) in self.histories.iter_mut().filter(|(key, history)| {
            key.chat == chat
                && key.thread != message.details.thread_root
                && key.thread.is_some()
                && history.iter().any(|candidate| candidate.id == message.id)
        }) {
            upsert_history_message(history, &message);
        }
        for chat_view in self
            .all_chats
            .iter_mut()
            .chain(self.view.chats.iter_mut())
            .filter(|candidate| candidate.id == chat)
        {
            chat_view.preview.clone_from(&message.body);
            chat_view.preview_sender = Some(message.sender.clone());
            chat_view.preview_sender_peer = message.details.sender_peer;
            chat_view.preview_timestamp.clone_from(&message.timestamp);
        }
        if self.active_chat_id() == Some(chat) {
            self.refresh_active_history_at(active_message, transcript_anchor);
        }
    }

    pub(super) fn delete_messages(&mut self, chat: Option<ChatId>, ids: &[MessageId]) {
        let active_message = self.active_message_id();
        let transcript_anchor = self.transcript_anchor_id();
        for (key, history) in &mut self.histories {
            if chat.is_none_or(|chat| key.chat == chat) {
                history.retain(|message| !ids.contains(&message.id));
            }
        }
        if chat.is_none_or(|chat| self.active_chat_id() == Some(chat)) {
            self.refresh_active_history_at(active_message, transcript_anchor);
        }
    }

    pub(super) fn apply_read_state(
        &mut self,
        chat: ChatId,
        max_id: MessageId,
        outgoing: bool,
        unread: Option<u32>,
    ) {
        if outgoing {
            for (key, history) in &mut self.histories {
                if key.chat == chat {
                    for message in history.iter_mut().filter(|message| {
                        message.direction == MessageDirection::Outgoing && message.id.0 <= max_id.0
                    }) {
                        message.delivery = DeliveryState::Read;
                    }
                }
            }
        }
        if let Some(unread) = unread {
            for chat_view in self
                .all_chats
                .iter_mut()
                .chain(self.view.chats.iter_mut())
                .filter(|candidate| candidate.id == chat)
            {
                chat_view.unread = unread;
            }
            self.refresh_folder_unread();
            if !outgoing {
                self.advance_unread_boundary(chat, max_id, unread);
            }
        }
        if self.active_chat_id() == Some(chat) {
            self.refresh_active_history();
        }
    }

    pub(super) fn refresh_folder_unread(&mut self) {
        for folder in &mut self.view.folders {
            folder.unread = self
                .all_chats
                .iter()
                .filter(|chat| chat.folders.contains(&folder.id))
                .fold(0_u32, |total, chat| total.saturating_add(chat.unread));
        }
    }
}

fn upsert_history_message(history: &mut Vec<MessageView>, message: &MessageView) {
    let mut found = false;
    history.retain_mut(|candidate| {
        if candidate.id != message.id {
            return true;
        }
        if found {
            return false;
        }
        candidate.clone_from(message);
        found = true;
        true
    });
    if !found {
        history.push(message.clone());
    }
}
