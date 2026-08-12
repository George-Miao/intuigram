use super::*;

impl App {
    pub(in crate::app) fn move_folder(&mut self, forward: bool) -> Option<Effect> {
        if self.view.focus != Focus::Chats || self.view.folders.is_empty() {
            return None;
        }
        let next = move_index(
            Some(self.view.active_folder),
            self.view.folders.len(),
            forward,
        )
        .unwrap_or(0);
        self.select_folder(next)
    }

    pub(in crate::app) fn select_folder(&mut self, index: usize) -> Option<Effect> {
        if index >= self.view.folders.len() || index == self.view.active_folder {
            return None;
        }
        let active_chat = self.active_chat_id();
        self.save_active_draft();
        self.save_transcript_anchor();
        self.clear_message_selection();
        self.view.active_folder = index;
        self.refresh_folder_chats(active_chat);
        self.restore_active_draft();
        self.view.active_thread = None;
        let transcript_anchor = self
            .active_history_key()
            .and_then(|key| self.transcript_anchors.get(&key).copied());
        self.view.active_message = None;
        self.view.transcript_anchor = None;
        self.refresh_active_history_at(None, transcript_anchor);
        Some(self.selection_effect())
    }

    pub(in crate::app) fn selection_view(&self) -> SelectionView {
        SelectionView {
            folder: self
                .view
                .folders
                .get(self.view.active_folder)
                .map_or(0, |folder| folder.id),
            chat: self.active_chat_id(),
            message: self
                .view
                .active_message
                .or(self.view.transcript_anchor)
                .and_then(|index| self.view.messages.get(index))
                .map(|message| message.id),
        }
    }

    pub(in crate::app) fn selection_effect(&self) -> Effect {
        let selection = self.selection_view();
        Effect::SaveSelection {
            folder: selection.folder,
            chat: selection.chat,
            message: selection.message,
            transcript_anchors: self.transcript_anchor_views(),
        }
    }

    pub(in crate::app) fn transcript_anchor_views(&self) -> Vec<TranscriptAnchorView> {
        let active_key = self.active_history_key();
        let active_message = self.selection_view().message;
        let mut anchors = self
            .transcript_anchors
            .iter()
            .filter(|(key, _)| Some(**key) != active_key || active_message.is_none())
            .map(|(key, message)| TranscriptAnchorView {
                chat: key.chat,
                thread: key.thread,
                saved_peer: key.saved_peer,
                message: *message,
            })
            .collect::<Vec<_>>();
        if let (Some(key), Some(message)) = (active_key, active_message) {
            anchors.push(TranscriptAnchorView {
                chat: key.chat,
                thread: key.thread,
                saved_peer: key.saved_peer,
                message,
            });
        }
        anchors.sort_unstable_by_key(|anchor| {
            (
                anchor.chat,
                anchor.saved_peer.unwrap_or(ChatId(0)),
                anchor.thread.unwrap_or(MessageId(0)),
            )
        });
        anchors
    }

    pub(in crate::app) fn refresh_folder_chats(&mut self, preferred: Option<ChatId>) {
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
}
