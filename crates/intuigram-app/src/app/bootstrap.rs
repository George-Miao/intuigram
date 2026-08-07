impl App {
    pub(super) fn empty() -> Self {
        Self {
            view: View {
                connection: ConnectionState::Connecting,
                account_name: "Intuigram".to_owned(),
                notification_identity: "telegram:unknown".to_owned(),
                folders: Vec::new(),
                active_folder: 0,
                chats: Vec::new(),
                active_chat: None,
                messages: Vec::new(),
                chat_loading: ChatLoadingState::Idle,
                pinned_messages: Vec::new(),
                active_message: None,
                selected_messages: Vec::new(),
                active_thread: None,
                transcript_anchor: None,
                unread_boundary: None,
                focus: Focus::Chats,
                composer: ComposerView::default(),
                search: None,
                save_as: None,
                attachment_path: None,
                has_newer_messages: false,
                help_open: false,
                folder_picker: None,
                delete_confirmation: None,
                forward_picker: None,
                reaction_picker: None,
                poll_vote: None,
                link_confirmation: None,
                downloads: Vec::new(),
                media_previews: Vec::new(),
                media_preview_loads: Vec::new(),
                poll_composer: false,
                notice: None,
                animation_frame: 0,
                actions: Vec::new(),
            },
            all_chats: Vec::new(),
            drafts: HashMap::new(),
            histories: HashMap::new(),
            pinned_histories: HashMap::new(),
            projected_pin: false,
            transcript_anchors: HashMap::new(),
            unread_boundaries: HashMap::new(),
            history_loads: HistoryLoads::default(),
            media_preview_loads: MediaPreviewLoads::default(),
            next_local_message_id: 0,
            pending_drafts: HashMap::new(),
            saved_poll_draft: None,
            pending_polls: HashMap::new(),
        }
    }

    pub(super) fn apply_intent(&mut self, intent: Intent) -> Option<Effect> {
        match intent {
            Intent::Insert(text) => {
                if let Some(attachment) = &mut self.view.attachment_path {
                    attachment.path.push_str(&text);
                    return None;
                } else if let Some(save_as) = &mut self.view.save_as {
                    save_as.destination.push_str(&text);
                    return None;
                } else if let Some(search) = &mut self.view.search {
                    search.query.push_str(&text);
                } else if self.view.active_chat.is_some() && self.view.focus != Focus::Chats {
                    self.focus_composer_at_anchor();
                    self.insert_composer_text(&text);
                }
                self.draft_effect()
            }
            Intent::Backspace => {
                if let Some(attachment) = &mut self.view.attachment_path {
                    attachment.path.pop();
                    return None;
                } else if let Some(save_as) = &mut self.view.save_as {
                    save_as.destination.pop();
                    return None;
                } else if let Some(search) = &mut self.view.search {
                    search.query.pop();
                } else if self.view.focus == Focus::Composer {
                    self.backspace_composer();
                }
                self.draft_effect()
            }
            Intent::MoveComposerCursor(movement) => {
                self.move_composer_cursor(movement);
                None
            }
            Intent::Activate(target) => self.activate(target),
            Intent::Animate => {
                if self.view.has_pending_effort() {
                    self.view.animation_frame = self.view.animation_frame.wrapping_add(1);
                }
                None
            }
            Intent::Action(action) => self.apply_action(action),
        }
    }

    pub(super) fn replace_bootstrap(&mut self, bootstrap: Bootstrap) {
        let restored_selection = bootstrap.restored_selection;
        let restored_anchors = bootstrap.transcript_anchors;
        self.view.connection = bootstrap.connection;
        self.view.account_name = bootstrap.account_name;
        self.view.notification_identity = bootstrap.notification_identity;
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
                        cursor: draft.text.len(),
                        text: draft.text,
                        reply_to: draft.reply_to,
                        editing: None,
                        attachments: Vec::new(),
                    },
                )
            })
            .collect();
        let default_folder = self
            .view
            .folders
            .iter()
            .position(|folder| folder.id == 0)
            .unwrap_or(0);
        let restored_folder = restored_selection.and_then(|selection| {
            self.view
                .folders
                .iter()
                .position(|folder| folder.id == selection.folder)
        });
        self.view.active_folder = restored_folder.unwrap_or(default_folder);
        self.refresh_folder_chats(None);
        self.view.active_chat = match restored_selection {
            None => (!self.view.chats.is_empty()).then_some(0),
            Some(selection) if restored_folder.is_some() => match selection.chat {
                None => None,
                Some(chat) => self
                    .view
                    .chats
                    .iter()
                    .position(|candidate| candidate.id == chat),
            },
            Some(_) => None,
        };
        if restored_selection.is_some_and(|selection| {
            restored_folder.is_some() && selection.chat.is_some() && self.view.active_chat.is_none()
        }) {
            self.view.active_folder = default_folder;
            self.refresh_folder_chats(None);
            self.view.active_chat = None;
        }
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
        self.pinned_histories = self
            .histories
            .iter()
            .filter(|(key, _)| key.thread.is_none())
            .map(|(key, messages)| {
                (
                    key.chat,
                    messages
                        .iter()
                        .filter(|message| message.details.pinned)
                        .cloned()
                        .collect(),
                )
            })
            .collect();
        for projection in bootstrap.pinned_messages {
            self.pinned_histories
                .insert(projection.chat, projection.messages);
        }
        self.transcript_anchors = restored_anchors
            .into_iter()
            .filter_map(|anchor| {
                let key = HistoryKey {
                    chat: anchor.chat,
                    thread: anchor.thread,
                };
                self.histories
                    .get(&key)
                    .is_some_and(|history| {
                        history.iter().any(|message| message.id == anchor.message)
                    })
                    .then_some((key, anchor.message))
            })
            .collect();
        if let Some(chat) = self.active_chat_id() {
            self.histories
                .insert(HistoryKey { chat, thread: None }, bootstrap.messages);
        }
        self.rebuild_unread_boundaries();
        self.view.active_message = None;
        self.view.selected_messages.clear();
        self.view.delete_confirmation = None;
        self.view.forward_picker = None;
        self.view.reaction_picker = None;
        self.view.poll_vote = None;
        self.view.poll_composer = false;
        self.saved_poll_draft = None;
        self.view.active_thread = None;
        self.view.transcript_anchor = None;
        self.refresh_active_history();
        if let Some(message) = restored_selection.and_then(|selection| selection.message)
            && let Some(index) = self
                .view
                .messages
                .iter()
                .position(|candidate| candidate.id == message)
        {
            self.view.transcript_anchor = Some(index);
        }
        self.restore_active_draft();
        self.reset_background_history();
        self.media_preview_loads = MediaPreviewLoads::default();
        self.view.media_preview_loads.clear();
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
        let selected_messages = self.view.selected_messages.clone();
        let transcript_anchor = self.transcript_anchor_id();
        let focus = self.view.focus;
        let drafts = std::mem::take(&mut self.drafts);
        let histories = std::mem::take(&mut self.histories);
        let pinned_histories = std::mem::take(&mut self.pinned_histories);

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
        self.view.selected_messages = selected_messages;
        self.view.focus = focus;
        self.view.notice = None;
        self.refresh_active_history_at(active_message, transcript_anchor);
        self.restore_active_draft();
        self.reset_background_history();
    }
}
use super::*;
