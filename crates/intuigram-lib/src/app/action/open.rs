use super::*;

impl App {
    pub(super) fn open_active_context(&mut self) -> Option<Effect> {
        if self.view.focus == Focus::Topics {
            return self.open_active_topic();
        }
        if self.view.focus == Focus::SavedDialogs {
            return self.open_active_saved_dialog();
        }
        let chat = self.active_chat_id()?;
        if self.active_chat_has_saved_dialogs() {
            return self.open_saved_dialogs(chat);
        }
        if self.active_chat_has_topics() {
            return self.open_topics(chat);
        }
        self.focus_composer_at_anchor();
        self.queue_active_media_previews();
        self.queue_visible_avatars();
        self.defer_active_read();
        self.request_chat_load(chat)
            .or_else(|| self.request_next_small_media())
            .or_else(|| {
                (!self.history_load_is_active())
                    .then(|| self.take_pending_read())
                    .flatten()
            })
    }
}
