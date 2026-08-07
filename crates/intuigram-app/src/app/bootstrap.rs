impl App {
    pub(super) fn apply_intent(&mut self, intent: Intent) -> Option<Effect> {
        match intent {
            Intent::Insert(text) => {
                if let Some(search) = &mut self.view.search {
                    search.query.push_str(&text);
                } else if self.view.active_chat.is_some() && self.view.focus != Focus::Chats {
                    self.focus_composer_at_anchor();
                    self.view.composer.text.push_str(&text);
                }
                self.draft_effect()
            }
            Intent::Backspace => {
                if let Some(search) = &mut self.view.search {
                    search.query.pop();
                } else if self.view.focus == Focus::Composer {
                    self.view.composer.text.pop();
                }
                self.draft_effect()
            }
            Intent::Action(action) => self.apply_action(action),
        }
    }

    pub(super) fn replace_bootstrap(&mut self, bootstrap: Bootstrap) {
        self.view.connection = bootstrap.connection;
        self.view.account_name = bootstrap.account_name;
        self.view.folders = bootstrap.folders;
        self.all_chats = bootstrap.chats;
        self.drafts = bootstrap
            .drafts
            .into_iter()
            .map(|draft| {
                (
                    HistoryKey {
                        chat: draft.chat,
                        thread: draft.thread_root,
                    },
                    ComposerView {
                        text: draft.text,
                        reply_to: draft.reply_to,
                        editing: None,
                        attachments: Vec::new(),
                    },
                )
            })
            .collect();
        self.refresh_folder_chats(None);
        self.view.active_chat = (!self.view.chats.is_empty()).then_some(0);
        self.histories = bootstrap
            .histories
            .into_iter()
            .map(|history| {
                (
                    HistoryKey {
                        chat: history.chat,
                        thread: history.thread_root,
                    },
                    history.messages,
                )
            })
            .collect();
        self.transcript_anchors.clear();
        if let Some(chat) = self.active_chat_id() {
            self.histories
                .insert(HistoryKey { chat, thread: None }, bootstrap.messages);
        }
        self.view.active_message = None;
        self.view.delete_confirmation = None;
        self.view.forward_picker = None;
        self.view.reaction_picker = None;
        self.view.poll_vote = None;
        self.view.poll_composer = false;
        self.saved_poll_draft = None;
        self.view.active_thread = None;
        self.view.transcript_anchor = None;
        self.refresh_active_history();
        self.restore_active_draft();
        self.reset_background_history();
        self.media_preview_loads = MediaPreviewLoads::default();
    }

    pub(super) fn merge_restored_connection(&mut self, bootstrap: Bootstrap) {
        let active_chat = self.active_chat_id();
        let active_folder = self
            .view
            .folders
            .get(self.view.active_folder)
            .map(|folder| folder.id);
        let active_thread = self.view.active_thread;
        let active_message = self.active_message_id();
        let transcript_anchor = self.transcript_anchor_id();
        let focus = self.view.focus;
        let drafts = std::mem::take(&mut self.drafts);
        let histories = std::mem::take(&mut self.histories);

        self.replace_bootstrap(bootstrap);

        self.drafts.extend(drafts);
        for pending in self.pending_drafts.values() {
            self.drafts.remove(&pending.history);
        }
        for (key, messages) in histories {
            let restored = self.histories.entry(key).or_default();
            for message in messages.into_iter().filter(|message| {
                matches!(
                    message.delivery,
                    DeliveryState::Pending | DeliveryState::Failed
                )
            }) {
                restored.retain(|candidate| candidate.id != message.id);
                restored.push(message);
            }
        }
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
        self.view.focus = focus;
        self.view.notice = None;
        self.refresh_active_history_at(active_message, transcript_anchor);
        self.restore_active_draft();
        self.reset_background_history();
    }
}
use super::*;
