use super::*;

impl App {
    pub(super) fn move_folder(&mut self, forward: bool) -> Option<Effect> {
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

    pub(super) fn select_folder(&mut self, index: usize) -> Option<Effect> {
        if index >= self.view.folders.len() || index == self.view.active_folder {
            return None;
        }
        let active_chat = self.active_chat_id();
        self.save_active_draft();
        self.save_transcript_anchor();
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

    pub(super) fn selection_view(&self) -> SelectionView {
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

    pub(super) fn selection_effect(&self) -> Effect {
        let selection = self.selection_view();
        Effect::SaveSelection {
            folder: selection.folder,
            chat: selection.chat,
            message: selection.message,
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
}
