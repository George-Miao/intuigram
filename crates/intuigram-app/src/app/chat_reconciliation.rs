use super::*;

impl App {
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
        let mut replaced = false;
        for (key, history) in self
            .histories
            .iter_mut()
            .filter(|(key, _)| key.chat == chat)
        {
            if let Some(existing) = history
                .iter_mut()
                .find(|candidate| candidate.id == message.id)
            {
                existing.clone_from(&message);
                replaced = true;
            } else if key.thread == message.details.thread_root {
                history.push(message.clone());
            }
        }
        if !replaced {
            self.histories
                .entry(HistoryKey { chat, thread: None })
                .or_default()
                .push(message.clone());
            if let Some(root) = message.details.thread_root {
                self.histories
                    .entry(HistoryKey {
                        chat,
                        thread: Some(root),
                    })
                    .or_default()
                    .push(message.clone());
            }
        }
        for chat_view in self
            .all_chats
            .iter_mut()
            .chain(self.view.chats.iter_mut())
            .filter(|candidate| candidate.id == chat)
        {
            chat_view.preview.clone_from(&message.body);
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
