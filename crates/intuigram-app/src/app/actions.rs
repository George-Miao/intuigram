impl App {
    pub(super) fn apply_action(&mut self, action: Action) -> Option<Effect> {
        if self.view.save_as.is_some() && action != Action::Quit {
            return self.apply_save_as_action(action);
        }
        if self.view.attachment_path.is_some() && action != Action::Quit {
            return match action {
                Action::ConfirmAttachment => self.confirm_attachment(),
                Action::Cancel | Action::Attach => {
                    self.view.attachment_path = None;
                    None
                }
                _ => None,
            };
        }
        if self.view.reaction_picker.is_some() && action != Action::Quit {
            return self.apply_reaction_picker(action);
        }
        if self.view.poll_vote.is_some() && action != Action::Quit {
            return self.apply_poll_vote(action);
        }
        if self.view.forward_picker.is_some() && action != Action::Quit {
            return self.apply_forward_picker(action);
        }
        if self.view.delete_confirmation.is_some() && action != Action::Quit {
            return self.apply_delete_confirmation(action);
        }
        if self.view.link_confirmation.is_some() && action != Action::Quit {
            return match action {
                Action::ConfirmOpenLink => self.confirm_active_link(),
                Action::Cancel | Action::OpenLink => {
                    self.view.link_confirmation = None;
                    None
                }
                _ => None,
            };
        }
        if self.view.folder_picker.is_some() && action != Action::Quit {
            return self.apply_folder_picker_action(action);
        }
        match action {
            Action::Quit => Some(Effect::Quit),
            Action::Help => {
                self.view.help_open = !self.view.help_open;
                None
            }
            Action::Reconnect if self.view.connection == ConnectionState::ReconnectCooldown => {
                self.view.connection = ConnectionState::Connecting;
                self.view.notice = None;
                Some(Effect::Reconnect)
            }
            Action::Reconnect => None,
            Action::MoveUp => self.move_chat(false),
            Action::MoveDown => self.move_chat(true),
            Action::PreviousFolder => self.move_folder(false),
            Action::NextFolder => self.move_folder(true),
            Action::ManageFolders => {
                self.open_folder_picker();
                None
            }
            Action::ToggleFolderMembership => None,
            Action::Open => {
                if let Some(chat) = self.active_chat_id() {
                    self.focus_composer_at_anchor();
                    self.queue_active_media_previews();
                    return self
                        .request_chat_load(chat)
                        .or_else(|| self.request_next_media_preview());
                }
                None
            }
            Action::Compose => {
                if self.view.active_chat.is_some() {
                    self.focus_composer_at_anchor();
                }
                None
            }
            Action::Reply => {
                self.view.composer.reply_to = self.active_message_id();
                if self.view.composer.reply_to.is_some() {
                    self.focus_composer_at_anchor();
                }
                self.draft_effect()
            }
            Action::Edit => {
                self.begin_edit();
                None
            }
            Action::EditPrevious => {
                self.begin_previous_edit();
                None
            }
            Action::Delete => {
                self.view.delete_confirmation = self.active_message_id();
                None
            }
            Action::ConfirmDelete => None,
            Action::Forward => {
                self.open_forward_picker();
                None
            }
            Action::ConfirmForward => None,
            Action::React => {
                self.open_reaction_picker();
                None
            }
            Action::ConfirmReaction => None,
            Action::VotePoll => {
                self.open_poll_vote();
                None
            }
            Action::TogglePollChoice | Action::ConfirmPollVote => None,
            Action::OpenLink => self.open_active_link(),
            Action::ConfirmOpenLink => None,
            Action::DownloadMedia => self.download_active_media(),
            Action::SaveAs => {
                self.open_save_as();
                None
            }
            Action::ConfirmSaveAs => self.confirm_save_as(),
            Action::OpenDownload => self.open_download(),
            Action::SaveEdit => self.save_edit(),
            Action::OpenThread => self.open_thread(),
            Action::NavigatePinned => {
                self.navigate_pinned();
                None
            }
            Action::TogglePin => self.toggle_active_pin(),
            Action::Paste => self.active_history_key().map(|key| Effect::ReadClipboard {
                chat: key.chat,
                thread_root: key.thread,
            }),
            Action::Attach => {
                self.view.attachment_path = Some(AttachmentPathView {
                    path: String::new(),
                });
                None
            }
            Action::ConfirmAttachment => self.confirm_attachment(),
            Action::CreatePoll => {
                self.begin_poll();
                None
            }
            Action::SendPoll => self.send_poll(),
            Action::TargetPreviousMessage => {
                self.target_previous_message();
                Some(self.selection_effect())
            }
            Action::TargetNextMessage => {
                self.target_next_message();
                Some(self.selection_effect())
            }
            Action::Search => {
                let scope = if self.view.focus == Focus::Chats {
                    SearchScope::Account
                } else {
                    SearchScope::Chat
                };
                self.view.search = Some(SearchView {
                    scope,
                    query: String::new(),
                });
                self.view.focus = Focus::Search;
                None
            }
            Action::Cancel => {
                if self.view.help_open {
                    self.view.help_open = false;
                } else if let Some(search) = self.view.search.take() {
                    self.view.focus = match search.scope {
                        SearchScope::Account => Focus::Chats,
                        SearchScope::Chat => Focus::Composer,
                    };
                } else if self.view.composer.editing.is_some() {
                    self.cancel_edit();
                } else if self.view.poll_composer {
                    self.cancel_poll();
                } else if self.view.composer.reply_to.take().is_some() {
                    self.view.focus = Focus::Composer;
                    return self.draft_effect();
                } else if self.view.focus == Focus::Transcript {
                    self.focus_composer_at_anchor();
                } else if self.view.focus == Focus::Composer {
                    if self.view.active_thread.is_some() {
                        self.leave_thread();
                    } else {
                        self.view.focus = Focus::Chats;
                    }
                }
                None
            }
            Action::JumpEarliest => {
                self.view.active_message = (!self.view.messages.is_empty()).then_some(0);
                self.view.transcript_anchor = self.view.active_message;
                self.view.focus = Focus::Transcript;
                Some(self.selection_effect())
            }
            Action::JumpLatest => {
                self.refresh_active_history();
                self.view.active_message = self.view.messages.len().checked_sub(1);
                self.view.transcript_anchor = self.view.active_message;
                self.view.has_newer_messages = false;
                self.view.focus = Focus::Transcript;
                Some(self.selection_effect())
            }
            Action::Send => self.send_message(),
            Action::Newline => {
                if self.view.focus == Focus::Composer {
                    self.insert_composer_text("\n");
                }
                self.draft_effect()
            }
        }
    }

    fn confirm_attachment(&mut self) -> Option<Effect> {
        let path = self.view.attachment_path.take()?.path;
        let key = self.active_history_key()?;
        if path.trim().is_empty() {
            self.view.notice = Some("Attachment path must not be empty".to_owned());
            return None;
        }
        Some(Effect::SelectAttachment {
            chat: key.chat,
            thread_root: key.thread,
            path,
        })
    }

    pub(super) fn open_folder_picker(&mut self) {
        if self.active_chat_id().is_some() {
            self.view.folder_picker = (self.view.folders.len() > 1).then_some(1);
        }
    }

    pub(super) fn apply_folder_picker_action(&mut self, action: Action) -> Option<Effect> {
        match action {
            Action::MoveUp => {
                let selected = self.view.folder_picker.unwrap_or(1);
                self.view.folder_picker = Some(selected.saturating_sub(1).max(1));
                None
            }
            Action::MoveDown => {
                let selected = self.view.folder_picker.unwrap_or(1);
                self.view.folder_picker = Some((selected + 1).min(self.view.folders.len() - 1));
                None
            }
            Action::ToggleFolderMembership => {
                let chat = self.active_chat_id()?;
                let folder = self
                    .view
                    .folder_picker
                    .and_then(|index| self.view.folders.get(index))?
                    .id;
                let included = !self
                    .all_chats
                    .iter()
                    .find(|candidate| candidate.id == chat)
                    .is_some_and(|candidate| candidate.folders.contains(&folder));
                self.view.folder_picker = None;
                Some(Effect::SetChatFolder {
                    chat,
                    folder,
                    included,
                })
            }
            Action::Cancel | Action::ManageFolders => {
                self.view.folder_picker = None;
                None
            }
            _ => None,
        }
    }

    pub(super) fn apply_folder_membership(
        &mut self,
        chat: ChatId,
        folder: i32,
        included: bool,
    ) -> Option<Effect> {
        let active_chat = self.active_chat_id();
        self.save_active_draft();
        self.save_transcript_anchor();
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
use super::*;
