use super::*;

impl App {
    pub(super) fn scroll(
        &mut self,
        target: ScrollTarget,
        direction: ScrollDirection,
    ) -> Option<Effect> {
        match target {
            ScrollTarget::Chats => {
                self.view.focus = Focus::Chats;
                self.move_chat(direction == ScrollDirection::Down)
            }
            ScrollTarget::Transcript => {
                let previous = (self.view.active_message, self.view.transcript_anchor);
                if self.view.focus == Focus::Chats {
                    self.view.focus = Focus::Composer;
                }
                match direction {
                    ScrollDirection::Up => self.target_previous_message(),
                    ScrollDirection::Down => self.target_next_message(),
                }
                (previous != (self.view.active_message, self.view.transcript_anchor))
                    .then(|| self.selection_effect())
            }
        }
    }

    pub(super) fn activate(&mut self, target: ActivationTarget) -> Option<Effect> {
        match target {
            ActivationTarget::Folder(folder) => self.activate_folder(folder),
            ActivationTarget::Chat(chat) => self.activate_chat(chat),
            ActivationTarget::Message(message) => {
                self.activate_message(message);
                Some(self.selection_effect())
            }
            ActivationTarget::Composer => {
                if self.active_chat_id().is_some() {
                    self.focus_composer_at_anchor();
                }
                None
            }
        }
    }

    fn activate_folder(&mut self, folder: i32) -> Option<Effect> {
        self.view.focus = Focus::Chats;
        let index = self
            .view
            .folders
            .iter()
            .position(|candidate| candidate.id == folder)?;
        self.select_folder(index)
    }

    fn activate_chat(&mut self, chat: ChatId) -> Option<Effect> {
        self.view.focus = Focus::Chats;
        let index = self
            .view
            .chats
            .iter()
            .position(|candidate| candidate.id == chat)?;
        if self.view.active_chat == Some(index) {
            return None;
        }
        self.save_active_draft();
        self.save_transcript_anchor();
        self.clear_message_selection();
        self.view.active_thread = None;
        self.view.chat_scroll_direction =
            self.view
                .active_chat
                .map_or(ScrollDirection::Down, |active| {
                    if index >= active {
                        ScrollDirection::Down
                    } else {
                        ScrollDirection::Up
                    }
                });
        self.view.active_chat = Some(index);
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
        self.request_chat_load(chat)
            .or_else(|| Some(self.selection_effect()))
    }

    fn activate_message(&mut self, message: MessageId) {
        let Some(index) = self
            .view
            .messages
            .iter()
            .position(|candidate| candidate.id == message)
        else {
            return;
        };
        if self.view.focus == Focus::Composer {
            self.save_active_draft();
        }
        self.view.active_message = Some(index);
        self.view.transcript_anchor = Some(index);
        self.view.focus = Focus::Transcript;
    }
}
