use super::*;

impl App {
    pub(super) fn merge_restored_connection(&mut self, bootstrap: Bootstrap) {
        let active_chat = self.active_chat_id();
        let active_history = self.active_history_key();
        let composer = self.view.composer.clone();
        let active_folder = self
            .view
            .folders
            .get(self.view.active_folder)
            .map(|folder| folder.id);
        let active_thread = self.view.active_thread;
        let active_saved_peer = self.view.active_saved_peer;
        let active_topic = self.active_topic_id();
        let active_message = self.active_message_id();
        let selected_messages = self.view.selected_messages.clone();
        let transcript_anchor = self.transcript_anchor_id();
        let focus = self.view.focus;
        let drafts = std::mem::take(&mut self.drafts);
        let histories = std::mem::take(&mut self.histories);
        let topic_lists = std::mem::take(&mut self.topic_lists);
        let saved_dialog_lists = std::mem::take(&mut self.saved_dialog_lists);
        let pinned_histories = std::mem::take(&mut self.pinned_histories);

        self.replace_bootstrap(bootstrap);

        self.drafts.extend(drafts);
        self.topic_lists.extend(topic_lists);
        for (chat, dialogs) in saved_dialog_lists {
            self.saved_dialog_lists.entry(chat).or_insert(dialogs);
        }
        for pending in self.pending_drafts.values() {
            self.drafts.remove(&pending.history);
        }
        for (key, messages) in histories {
            let restored = self.histories.entry(key).or_default();
            for message in messages.into_iter().filter(|message| {
                matches!(
                    message.delivery,
                    DeliveryState::Saving | DeliveryState::Pending | DeliveryState::Failed
                )
            }) {
                restored.retain(|candidate| candidate.id != message.id);
                restored.push(message);
            }
        }
        self.pinned_histories.extend(pinned_histories);
        if let Some(folder) = active_folder
            && let Some(index) = self
                .view
                .folders
                .iter()
                .position(|candidate| candidate.id == folder)
        {
            self.view.active_folder = index;
        }
        self.refresh_folder_chats(active_chat);
        self.view.active_chat = active_chat
            .and_then(|chat| {
                self.view
                    .chats
                    .iter()
                    .position(|candidate| candidate.id == chat)
            })
            .or_else(|| (!self.view.chats.is_empty()).then_some(0));
        self.view.active_thread = active_thread.filter(|_| self.active_chat_id() == active_chat);
        self.restore_reconnected_topics(active_topic);
        self.restore_active_saved_dialogs();
        if self.active_chat_id() == active_chat
            && let Some(peer) = active_saved_peer
            && let Some(index) = self
                .view
                .saved_dialogs
                .iter()
                .position(|dialog| dialog.peer == peer)
        {
            self.view.active_saved_dialog = Some(index);
            self.view.active_saved_peer = Some(peer);
        }
        self.view.selected_messages = selected_messages;
        self.view.focus = focus;
        self.view.notice = None;
        self.refresh_active_history_at(active_message, transcript_anchor);
        if self.active_history_key() == active_history {
            self.view.composer = composer;
        } else {
            self.restore_active_draft();
        }
        self.reset_reconnected_history();
    }
}
