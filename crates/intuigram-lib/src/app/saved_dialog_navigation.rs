use super::*;

impl App {
    pub(super) fn active_chat_is_saved_messages(&self) -> bool {
        self.view
            .active_chat
            .and_then(|index| self.view.chats.get(index))
            .is_some_and(|chat| chat.kind == ChatKind::SavedMessages)
    }

    pub(super) fn active_chat_has_saved_dialogs(&self) -> bool {
        self.view
            .active_chat
            .and_then(|index| self.view.chats.get(index))
            .is_some_and(|chat| chat.kind == ChatKind::SavedMessages || chat.has_direct_messages)
    }

    pub(super) fn saved_history_is_read_only(&self) -> bool {
        self.active_chat_is_saved_messages()
            && self
                .view
                .active_saved_peer
                .is_some_and(|peer| self.active_chat_id() != Some(peer))
    }

    pub(super) fn restore_active_saved_dialogs(&mut self) {
        self.view.saved_dialogs = self
            .active_chat_id()
            .and_then(|chat| self.saved_dialog_lists.get(&chat))
            .cloned()
            .unwrap_or_default();
        self.view.active_saved_dialog = (!self.view.saved_dialogs.is_empty()).then_some(0);
        self.view.active_saved_peer = None;
        self.view.saved_dialogs_loading = false;
    }

    pub(super) fn open_saved_dialogs(&mut self, chat: ChatId) -> Option<Effect> {
        self.restore_active_saved_dialogs();
        self.view.focus = Focus::SavedDialogs;
        self.view.active_thread = None;
        self.view.saved_dialogs_loading = true;
        Some(Effect::LoadSavedDialogs(chat))
    }

    pub(super) fn apply_saved_dialog_event(&mut self, event: AdapterEvent) -> Option<Effect> {
        match event {
            AdapterEvent::SavedDialogsLoaded(list) => {
                let selected = (self.active_chat_id() == Some(list.chat))
                    .then(|| {
                        self.view
                            .active_saved_dialog
                            .and_then(|index| self.view.saved_dialogs.get(index))
                            .map(|dialog| dialog.peer)
                    })
                    .flatten();
                for dialog in &list.dialogs {
                    if let Some(draft) = &dialog.draft {
                        self.drafts
                            .entry(HistoryKey::saved(list.chat, dialog.peer))
                            .or_insert_with(|| ComposerView {
                                text: draft.text.clone(),
                                cursor: draft.text.len(),
                                reply_to: draft.reply_to,
                                editing: None,
                                attachments: Vec::new(),
                            });
                    }
                }
                self.saved_dialog_lists.insert(list.chat, list.dialogs);
                if self.active_chat_id() == Some(list.chat) {
                    self.view.saved_dialogs = self
                        .saved_dialog_lists
                        .get(&list.chat)
                        .cloned()
                        .unwrap_or_default();
                    self.view.active_saved_dialog = selected
                        .and_then(|peer| {
                            self.view
                                .saved_dialogs
                                .iter()
                                .position(|dialog| dialog.peer == peer)
                        })
                        .or_else(|| (!self.view.saved_dialogs.is_empty()).then_some(0));
                    self.view.saved_dialogs_loading = false;
                }
            }
            AdapterEvent::SavedDialogsLoadFailed(failure) => {
                if self.active_chat_id() == Some(failure.chat) {
                    self.view.saved_dialogs_loading = false;
                    self.view.notice = Some(failure.reason);
                }
            }
            _ => unreachable!("only Saved Messages dialog events are routed here"),
        }
        self.queue_visible_avatars();
        self.request_next_avatar()
    }

    pub(super) fn move_saved_dialog(&mut self, forward: bool) {
        self.view.active_saved_dialog = move_index(
            self.view.active_saved_dialog,
            self.view.saved_dialogs.len(),
            forward,
        );
    }

    pub(super) fn apply_saved_history_event(&mut self, event: AdapterEvent) -> Option<Effect> {
        let (key, loaded, failure) = match event {
            AdapterEvent::SavedHistoryLoaded {
                chat,
                peer,
                messages,
            } => (HistoryKey::saved(chat, peer), Some(messages), None),
            AdapterEvent::SavedHistoryLoadFailed { chat, peer, reason } => {
                (HistoryKey::saved(chat, peer), None, Some(reason))
            }
            _ => unreachable!("only Saved Messages history events are routed here"),
        };
        if let Some(messages) = loaded {
            self.store_loaded_history(key, messages);
            if self.active_history_key() == Some(key) {
                self.queue_active_media_previews();
                self.queue_visible_avatars();
                self.defer_active_read();
            }
            self.complete_history_load(key, true)
        } else {
            if self.active_history_key() == Some(key) {
                self.view.notice = failure;
            }
            self.complete_history_load(key, false)
        }
    }

    pub(super) fn replace_saved_dialog_lists(&mut self, lists: Vec<SavedDialogListView>) {
        self.saved_dialog_lists = lists
            .into_iter()
            .map(|list| (list.chat, list.dialogs))
            .collect();
    }

    pub(super) fn seed_saved_dialog_drafts(&mut self) {
        for (chat, dialogs) in &self.saved_dialog_lists {
            for dialog in dialogs {
                if let Some(draft) = &dialog.draft {
                    self.drafts
                        .entry(HistoryKey::saved(*chat, dialog.peer))
                        .or_insert_with(|| ComposerView {
                            text: draft.text.clone(),
                            cursor: draft.text.len(),
                            reply_to: draft.reply_to,
                            editing: None,
                            attachments: Vec::new(),
                        });
                }
            }
        }
    }

    pub(super) fn update_saved_dialog_from_message(
        &mut self,
        chat: ChatId,
        peer: ChatId,
        message: &MessageView,
        unread_increment: u32,
    ) {
        if let Some(dialogs) = self.saved_dialog_lists.get_mut(&chat) {
            update_dialog_message(dialogs, peer, message, unread_increment);
        }
        if self.active_chat_id() == Some(chat) {
            update_dialog_message(
                &mut self.view.saved_dialogs,
                peer,
                message,
                unread_increment,
            );
        }
    }

    pub(super) fn set_saved_dialog_unread(&mut self, chat: ChatId, peer: ChatId, unread: u32) {
        if let Some(dialogs) = self.saved_dialog_lists.get_mut(&chat) {
            set_dialog_unread(dialogs, peer, unread);
        }
        if self.active_chat_id() == Some(chat) {
            set_dialog_unread(&mut self.view.saved_dialogs, peer, unread);
        }
    }

    pub(super) fn open_active_saved_dialog(&mut self) -> Option<Effect> {
        let chat = self.active_chat_id()?;
        let peer = self
            .view
            .active_saved_dialog
            .and_then(|index| self.view.saved_dialogs.get(index))?
            .peer;
        self.clear_message_selection();
        self.view.active_saved_peer = Some(peer);
        self.view.active_thread = None;
        self.view.active_message = None;
        self.view.transcript_anchor = None;
        self.refresh_active_history();
        self.restore_active_draft();
        self.view.focus = if self.saved_history_is_read_only() {
            Focus::Transcript
        } else {
            Focus::Composer
        };
        self.request_history_load(HistoryKey::saved(chat, peer))
    }

    pub(super) fn leave_saved_dialog(&mut self) {
        self.save_transcript_anchor();
        self.clear_message_selection();
        self.view.active_saved_peer = None;
        self.view.active_message = None;
        self.view.transcript_anchor = None;
        self.view.messages.clear();
        self.view.pinned_messages.clear();
        self.view.composer = ComposerView::default();
        self.view.focus = Focus::SavedDialogs;
    }
}

fn update_dialog_message(
    dialogs: &mut [SavedDialogView],
    peer: ChatId,
    message: &MessageView,
    unread_increment: u32,
) {
    if let Some(dialog) = dialogs.iter_mut().find(|dialog| dialog.peer == peer) {
        dialog.preview.clone_from(&message.body);
        dialog.timestamp.clone_from(&message.timestamp);
        dialog.top_message = message.id;
        dialog.unread = dialog.unread.saturating_add(unread_increment);
    }
}

fn set_dialog_unread(dialogs: &mut [SavedDialogView], peer: ChatId, unread: u32) {
    if let Some(dialog) = dialogs.iter_mut().find(|dialog| dialog.peer == peer) {
        dialog.unread = unread;
        dialog.unread_mark = false;
    }
}
