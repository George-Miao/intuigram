impl App {
    pub(super) fn refresh_actions(&mut self) {
        let mut actions = vec![Action::Quit];
        if self.view.focus != Focus::Composer {
            actions.push(Action::Help);
        }
        if self.view.help_open {
            self.view.actions = vec![Action::Quit, Action::Help, Action::Cancel];
            return;
        }
        if self.view.save_as.is_some() {
            self.view.actions = vec![Action::Quit, Action::ConfirmSaveAs, Action::Cancel];
            return;
        }
        if self.view.attachment_path.is_some() {
            self.view.actions = vec![Action::Quit, Action::ConfirmAttachment, Action::Cancel];
            return;
        }
        if let Some(manager) = &self.view.scheduled {
            if manager.pending {
                self.view.actions = vec![Action::Quit];
                return;
            }
            if manager.confirmation.is_some() {
                self.view.actions = vec![Action::Quit, Action::ConfirmScheduled, Action::Cancel];
                return;
            }
            if let Some(editor) = &manager.editor {
                let mut actions = vec![
                    Action::Quit,
                    Action::MoveUp,
                    Action::MoveDown,
                    Action::Cancel,
                ];
                let valid = match editor.operation {
                    ScheduledEditorOperation::Create => {
                        !editor.text.trim().is_empty()
                            && ScheduledDeliveryView::parse(&editor.delivery).is_some()
                    }
                    ScheduledEditorOperation::Edit(_) => !editor.text.trim().is_empty(),
                    ScheduledEditorOperation::Reschedule(_) => {
                        ScheduledDeliveryView::parse(&editor.delivery).is_some()
                    }
                };
                if valid {
                    actions.push(Action::SaveScheduled);
                }
                self.view.actions = actions;
                return;
            }
            let mut actions = vec![
                Action::Quit,
                Action::MoveUp,
                Action::MoveDown,
                Action::NewScheduled,
                Action::Cancel,
            ];
            if !manager.messages.is_empty() {
                actions.extend([
                    Action::EditScheduled,
                    Action::RescheduleScheduled,
                    Action::DeleteScheduled,
                    Action::SendScheduledNow,
                ]);
            }
            self.view.actions = actions;
            return;
        }
        if let Some(composer) = &self.view.rich_media {
            if composer.pending {
                self.view.actions = vec![Action::Quit];
                return;
            }
            let mut actions = vec![
                Action::Quit,
                Action::MoveUp,
                Action::MoveDown,
                Action::Cancel,
            ];
            let can_choose = match &composer.mode {
                RichMediaComposerMode::Menu => true,
                RichMediaComposerMode::Library { items, .. } => !items.is_empty(),
                RichMediaComposerMode::File { path, .. } => !path.trim().is_empty(),
                RichMediaComposerMode::Recording {
                    seconds, device, ..
                } => {
                    seconds.parse::<u32>().is_ok_and(|value| value > 0) && !device.trim().is_empty()
                }
                RichMediaComposerMode::Contact {
                    phone, first_name, ..
                } => !phone.trim().is_empty() && !first_name.trim().is_empty(),
            };
            if can_choose {
                actions.push(Action::ChooseRichMedia);
            }
            if matches!(composer.mode, RichMediaComposerMode::File { .. }) && composer.selected == 1
            {
                actions.push(Action::CycleRichMediaKind);
            }
            self.view.actions = actions;
            return;
        }
        if let Some(manager) = &self.view.folder_manager {
            self.view.actions = if manager.pending {
                vec![Action::Quit]
            } else if manager.delete_confirmation.is_some() {
                vec![Action::Quit, Action::ConfirmDeleteFolder, Action::Cancel]
            } else if let Some(editor) = &manager.editor {
                let mut actions = vec![
                    Action::Quit,
                    Action::MoveUp,
                    Action::MoveDown,
                    Action::Cancel,
                ];
                if !editor.title.trim().is_empty() {
                    actions.push(Action::SaveFolder);
                }
                if editor.rules.is_some() && editor.selected > 0 {
                    actions.push(Action::ToggleFolderRule);
                }
                actions
            } else {
                let mut actions = vec![
                    Action::Quit,
                    Action::MoveUp,
                    Action::MoveDown,
                    Action::CreateFolder,
                    Action::Cancel,
                ];
                if let Some(details) = self.view.folder_details.get(manager.selected) {
                    actions.extend([
                        Action::EditFolder,
                        Action::ReorderFolderUp,
                        Action::ReorderFolderDown,
                        Action::DeleteFolder,
                    ]);
                    if details.shareable {
                        actions.push(Action::ShareFolder);
                    }
                }
                actions
            };
            return;
        }
        if self.view.account_confirmation.is_some() {
            self.view.actions = vec![
                Action::Quit,
                Action::ConfirmAccountOperation,
                Action::Cancel,
            ];
            return;
        }
        if self.view.account_picker.is_some() {
            let mut picker_actions = vec![
                Action::Quit,
                Action::MoveUp,
                Action::MoveDown,
                Action::ConfirmAccount,
                Action::RemoveAccountLocally,
                Action::Cancel,
            ];
            let selected = self
                .view
                .account_picker
                .and_then(|index| self.view.accounts.get(index));
            if self.view.connection == ConnectionState::Connected
                && selected.is_some_and(|account| account.active)
            {
                picker_actions.push(Action::LogoutAccount);
            }
            self.view.actions = picker_actions;
            return;
        }
        if self.view.folder_picker.is_some() {
            self.view.actions = vec![
                Action::Quit,
                Action::MoveUp,
                Action::MoveDown,
                Action::ToggleFolderMembership,
                Action::Cancel,
            ];
            return;
        }
        if self.view.delete_confirmation.is_some() {
            self.view.actions = vec![Action::Quit, Action::ConfirmDelete, Action::Cancel];
            return;
        }
        if self.view.link_confirmation.is_some() {
            self.view.actions = vec![Action::Quit, Action::ConfirmOpenLink, Action::Cancel];
            return;
        }
        if self.view.forward_picker.is_some() {
            self.view.actions = vec![
                Action::Quit,
                Action::MoveUp,
                Action::MoveDown,
                Action::ConfirmForward,
                Action::Cancel,
            ];
            return;
        }
        if self.view.reaction_picker.is_some() {
            self.view.actions = vec![
                Action::Quit,
                Action::MoveUp,
                Action::MoveDown,
                Action::ConfirmReaction,
                Action::Cancel,
            ];
            return;
        }
        if self.view.poll_vote.is_some() {
            self.view.actions = vec![
                Action::Quit,
                Action::MoveUp,
                Action::MoveDown,
                Action::TogglePollChoice,
                Action::ConfirmPollVote,
                Action::Cancel,
            ];
            return;
        }
        match self.view.focus {
            Focus::Chats => {
                actions.extend([
                    Action::MoveUp,
                    Action::MoveDown,
                    Action::PreviousFolder,
                    Action::NextFolder,
                    Action::ManageFolders,
                    Action::ManageAccounts,
                    Action::Open,
                    Action::NavigatePinned,
                    Action::Search,
                ]);
                if self.view.connection == ConnectionState::Connected {
                    actions.push(Action::ManageFolderLifecycle);
                }
            }
            Focus::Transcript => {
                let has_cloud_message = !self.selected_message_ids().is_empty();
                actions.extend([
                    Action::NavigatePinned,
                    Action::TargetPreviousMessage,
                    Action::TargetNextMessage,
                    Action::Compose,
                    Action::Reply,
                    Action::OpenScheduled,
                    Action::OpenThread,
                    Action::Search,
                    Action::JumpEarliest,
                    Action::JumpLatest,
                    Action::React,
                    Action::TogglePin,
                    Action::ToggleMessageSelection,
                    Action::Cancel,
                ]);
                if has_cloud_message {
                    actions.extend([Action::Delete, Action::Forward]);
                }
                if self
                    .view
                    .active_message
                    .and_then(|index| self.view.messages.get(index))
                    .is_some_and(|message| {
                        message.direction == MessageDirection::Outgoing && message.id.0 > 0
                    })
                {
                    actions.push(Action::Edit);
                }
                if let Some(message) = self
                    .view
                    .active_message
                    .and_then(|index| self.view.messages.get(index))
                {
                    if active_link(message).is_some() {
                        actions.push(Action::OpenLink);
                    }
                    if message
                        .details
                        .media
                        .as_ref()
                        .is_some_and(|media| media.remote_id.is_some())
                    {
                        actions.push(Action::DownloadMedia);
                        actions.push(Action::SaveAs);
                    }
                    if message
                        .details
                        .media
                        .as_ref()
                        .is_some_and(|media| media.poll.as_ref().is_some_and(|poll| !poll.closed))
                    {
                        actions.push(Action::VotePoll);
                    }
                    if self.view.downloads.iter().any(|download| {
                        Some(download.chat) == self.active_chat_id()
                            && download.message == message.id
                    }) {
                        actions.push(Action::OpenDownload);
                    }
                }
            }
            Focus::Composer => {
                if self.view.composer.editing.is_some() {
                    actions.extend([Action::SaveEdit, Action::Newline, Action::Cancel]);
                } else if self.view.poll_composer {
                    actions.extend([Action::SendPoll, Action::Newline, Action::Cancel]);
                } else {
                    actions.extend([
                        Action::Send,
                        Action::Newline,
                        Action::Paste,
                        Action::Attach,
                        Action::OpenRichMedia,
                        Action::OpenScheduled,
                        Action::CreatePoll,
                        Action::Cancel,
                        Action::Search,
                        Action::TargetPreviousMessage,
                    ]);
                    if self.view.composer.text.is_empty()
                        && self.view.messages.iter().any(|message| {
                            message.direction == MessageDirection::Outgoing && message.id.0 > 0
                        })
                    {
                        actions.push(Action::EditPrevious);
                    }
                }
            }
            Focus::Search => actions.push(Action::Cancel),
        }
        if self.view.connection == ConnectionState::ReconnectCooldown {
            actions.push(Action::Reconnect);
        }
        if self.view.active_thread.is_some()
            || !self
                .view
                .pinned_messages
                .iter()
                .any(|message| message.details.pinned)
        {
            actions.retain(|action| *action != Action::NavigatePinned);
        }
        if !self
            .view
            .active_chat
            .and_then(|index| self.view.chats.get(index))
            .is_some_and(|chat| chat.can_pin_messages)
            || self
                .view
                .active_message
                .and_then(|index| self.view.messages.get(index))
                .is_none_or(|message| message.id.0 <= 0)
        {
            actions.retain(|action| *action != Action::TogglePin);
        }
        self.view.actions = actions;
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

pub(super) fn move_index(current: Option<usize>, length: usize, forward: bool) -> Option<usize> {
    if length == 0 {
        return None;
    }
    let current = current.unwrap_or(0).min(length - 1);
    Some(if forward {
        (current + 1).min(length - 1)
    } else {
        current.saturating_sub(1)
    })
}
use super::*;
