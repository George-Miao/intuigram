impl App {
    pub(super) fn empty() -> Self {
        Self {
            view: View {
                connection: ConnectionState::Connecting,
                account_name: "Intuigram".to_owned(),
                notification_identity: "telegram:unknown".to_owned(),
                accounts: Vec::new(),
                folders: Vec::new(),
                folder_details: Vec::new(),
                active_folder: 0,
                chats: Vec::new(),
                offline_chats: Vec::new(),
                active_chat: None,
                topics: Vec::new(),
                active_topic: None,
                topics_loading: false,
                saved_dialogs: Vec::new(),
                active_saved_dialog: None,
                active_saved_peer: None,
                saved_dialogs_loading: false,
                chat_scroll_direction: ScrollDirection::Down,
                messages: Vec::new(),
                outbox: Vec::new(),
                parent_messages: Vec::new(),
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
                action_menu: None,
                folder_picker: None,
                folder_manager: None,
                rich_media: None,
                scheduled: None,
                account_picker: None,
                account_confirmation: None,
                delete_confirmation: None,
                forward_picker: None,
                reaction_picker: None,
                poll_vote: None,
                todo_editor: None,
                link_confirmation: None,
                downloads: Vec::new(),
                media_previews: Vec::new(),
                image_popup: None,
                avatars: Vec::new(),
                avatar_loads: Vec::new(),
                media_preview_loads: Vec::new(),
                poll_composer: false,
                notice: None,
                animation_frame: 0,
                actions: Vec::new(),
            },
            all_chats: Vec::new(),
            muted_chats: HashSet::new(),
            drafts: HashMap::new(),
            histories: HashMap::new(),
            topic_lists: HashMap::new(),
            saved_dialog_lists: HashMap::new(),
            pinned_histories: HashMap::new(),
            projected_pin: false,
            transcript_anchors: HashMap::new(),
            unread_boundaries: HashMap::new(),
            history_loads: HistoryLoads::default(),
            media_preview_loads: MediaPreviewLoads::default(),
            offline_media: OfflineMedia::default(),
            avatar_peers: HashMap::new(),
            avatar_loads: AvatarLoads::default(),
            small_media_capacity: 5,
            next_local_message_id: 0,
            pending_drafts: HashMap::new(),
            saved_poll_draft: None,
            pending_polls: HashMap::new(),
        }
    }

    pub(super) fn apply_intent(&mut self, intent: Intent) -> Option<Effect> {
        match intent {
            Intent::Insert(text) => {
                if let Some(append) = self
                    .view
                    .todo_editor
                    .as_mut()
                    .and_then(|editor| editor.append.as_mut())
                {
                    append.push_str(&text);
                    return None;
                }
                if self.insert_scheduled_text(&text) {
                    return None;
                }
                if self.insert_rich_media_text(&text) {
                    return None;
                }
                if let Some(editor) = self
                    .view
                    .folder_manager
                    .as_mut()
                    .and_then(|manager| manager.editor.as_mut())
                    .filter(|editor| editor.selected == 0)
                {
                    editor.title.push_str(&text);
                    return None;
                } else if let Some(attachment) = &mut self.view.attachment_path {
                    attachment.path.push_str(&text);
                    return None;
                } else if let Some(save_as) = &mut self.view.save_as {
                    save_as.destination.push_str(&text);
                    return None;
                } else if let Some(search) = &mut self.view.search {
                    search.query.push_str(&text);
                } else if self.view.active_chat.is_some()
                    && !matches!(
                        self.view.focus,
                        Focus::Chats | Focus::Topics | Focus::SavedDialogs
                    )
                {
                    self.focus_composer_at_anchor();
                    self.insert_composer_text(&text);
                }
                self.draft_effect()
            }
            Intent::Backspace => {
                if let Some(append) = self
                    .view
                    .todo_editor
                    .as_mut()
                    .and_then(|editor| editor.append.as_mut())
                {
                    append.pop();
                    return None;
                }
                if self.backspace_scheduled_text() {
                    return None;
                }
                if self.backspace_rich_media_text() {
                    return None;
                }
                if let Some(editor) = self
                    .view
                    .folder_manager
                    .as_mut()
                    .and_then(|manager| manager.editor.as_mut())
                    .filter(|editor| editor.selected == 0)
                {
                    editor.title.pop();
                    return None;
                } else if let Some(attachment) = &mut self.view.attachment_path {
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
            Intent::SetComposerCursor(cursor) => {
                self.set_composer_cursor(cursor);
                None
            }
            Intent::Activate(target) => self.activate(target),
            Intent::Scroll(target, direction) => self.scroll(target, direction),
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
        let outbox = bootstrap.outbox;
        self.view.connection = bootstrap.connection;
        self.view.account_name = bootstrap.account_name;
        self.view.notification_identity = bootstrap.notification_identity;
        self.muted_chats = bootstrap.muted_chats.into_iter().collect();
        self.offline_media.replace(bootstrap.offline_chats);
        self.sync_offline_chat_view();
        self.view.accounts = bootstrap.accounts;
        self.view.folders = bootstrap.folders;
        self.view.folder_details = bootstrap.folder_details;
        self.avatar_peers = bootstrap
            .avatar_peers
            .into_iter()
            .map(|avatar| (avatar.peer, avatar.id))
            .collect();
        self.view
            .avatars
            .retain(|avatar| self.avatar_peers.get(&avatar.avatar.peer) == Some(&avatar.avatar.id));
        self.all_chats = bootstrap.chats;
        self.replace_topic_lists(bootstrap.topic_lists);
        let saved_dialog_lists = bootstrap.saved_dialog_lists;
        self.drafts = bootstrap
            .drafts
            .into_iter()
            .map(|draft| {
                (
                    HistoryKey::scoped(draft.chat, draft.thread_root, draft.saved_peer),
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
        self.replace_saved_dialog_lists(saved_dialog_lists);
        self.seed_saved_dialog_drafts();
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
        self.restore_active_topics();
        self.restore_active_saved_dialogs();
        self.histories = bootstrap
            .histories
            .into_iter()
            .map(|history| {
                (
                    HistoryKey::scoped(history.chat, history.thread_root, history.saved_peer),
                    history.messages,
                )
            })
            .collect();
        self.pinned_histories = self
            .histories
            .iter()
            .filter(|(key, _)| key.thread.is_none() && key.saved_peer.is_none())
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
                let key = HistoryKey::scoped(anchor.chat, anchor.thread, anchor.saved_peer);
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
                .insert(HistoryKey::root(chat), bootstrap.messages);
        }
        self.next_local_message_id = self
            .histories
            .values()
            .flatten()
            .map(|message| message.id.0)
            .filter(|id| *id < 0)
            .min()
            .unwrap_or(0);
        self.replace_outbox(outbox);
        self.rebuild_unread_boundaries();
        self.view.active_message = None;
        self.view.selected_messages.clear();
        self.view.action_menu = None;
        self.view.delete_confirmation = None;
        self.view.folder_manager = None;
        self.view.image_popup = None;
        self.view.rich_media = None;
        self.view.scheduled = None;
        self.view.account_picker = None;
        self.view.account_confirmation = None;
        self.view.forward_picker = None;
        self.view.reaction_picker = None;
        self.view.poll_vote = None;
        self.view.todo_editor = None;
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
        self.avatar_loads.reset_requests();
    }
}
use super::*;
