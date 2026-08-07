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
                    Action::Open,
                    Action::NavigatePinned,
                    Action::Search,
                ]);
            }
            Focus::Transcript => {
                actions.extend([
                    Action::NavigatePinned,
                    Action::TargetPreviousMessage,
                    Action::TargetNextMessage,
                    Action::Compose,
                    Action::Reply,
                    Action::OpenThread,
                    Action::Search,
                    Action::JumpEarliest,
                    Action::JumpLatest,
                    Action::Delete,
                    Action::Forward,
                    Action::React,
                    Action::TogglePin,
                    Action::Cancel,
                ]);
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
