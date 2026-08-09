impl App {
    pub(super) fn apply_action(&mut self, action: Action) -> Option<Effect> {
        if self.view.action_menu.is_some() && action != Action::Quit {
            return self.apply_action_menu(action);
        }
        if self.view.scheduled.is_some() && action != Action::Quit {
            return self.apply_scheduled_action(action);
        }
        if self.view.rich_media.is_some() && action != Action::Quit {
            return self.apply_rich_media_action(action);
        }
        if self.view.folder_manager.is_some() && action != Action::Quit {
            return self.apply_folder_manager_action(action);
        }
        if self.view.account_confirmation.is_some() && action != Action::Quit {
            return self.apply_account_confirmation(action);
        }
        if self.view.account_picker.is_some() && action != Action::Quit {
            return self.apply_account_picker_action(action);
        }
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
            Action::ManageFolderLifecycle => {
                self.open_folder_manager();
                None
            }
            Action::OpenRichMedia => {
                self.open_rich_media();
                None
            }
            Action::ChooseRichMedia | Action::CycleRichMediaKind => None,
            Action::OpenScheduled => self.open_scheduled(),
            Action::NewScheduled
            | Action::EditScheduled
            | Action::RescheduleScheduled
            | Action::DeleteScheduled
            | Action::SendScheduledNow
            | Action::SaveScheduled
            | Action::ConfirmScheduled => None,
            Action::CreateFolder
            | Action::EditFolder
            | Action::SaveFolder
            | Action::ReorderFolderUp
            | Action::ReorderFolderDown
            | Action::ShareFolder
            | Action::DeleteFolder
            | Action::ConfirmDeleteFolder
            | Action::ToggleFolderRule => None,
            Action::ManageAccounts => {
                self.open_account_picker();
                None
            }
            Action::ConfirmAccount
            | Action::LogoutAccount
            | Action::RemoveAccountLocally
            | Action::ConfirmAccountOperation => None,
            Action::ToggleFolderMembership => None,
            Action::Open => {
                if let Some(chat) = self.active_chat_id() {
                    self.focus_composer_at_anchor();
                    self.queue_active_media_previews();
                    self.queue_visible_avatars();
                    self.defer_active_read();
                    return self
                        .request_chat_load(chat)
                        .or_else(|| self.request_next_media_preview())
                        .or_else(|| self.request_next_avatar())
                        .or_else(|| {
                            (!self.history_load_is_active())
                                .then(|| self.take_pending_read())
                                .flatten()
                        });
                }
                None
            }
            Action::OpenActions => {
                self.open_action_menu();
                None
            }
            Action::ChooseAction => None,
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
            Action::Edit => self.begin_edit(),
            Action::EditPrevious => self.begin_previous_edit(),
            Action::Delete => {
                let messages = self.selected_message_ids();
                self.view.delete_confirmation = (!messages.is_empty()).then_some(messages);
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
            Action::ToggleMessageSelection => {
                self.toggle_message_selection();
                None
            }
            Action::Paste => self.active_history_key().map(|key| Effect::ReadClipboard {
                chat: key.chat,
                thread_root: key.thread,
            }),
            Action::Attach => self.active_history_key().map(|key| Effect::PickAttachment {
                chat: key.chat,
                thread_root: key.thread,
            }),
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
}
use super::*;
