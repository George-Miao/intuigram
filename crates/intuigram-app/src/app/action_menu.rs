use super::*;

impl App {
    pub(super) fn available_message_actions(&self) -> Vec<Action> {
        let Some(message) = self
            .view
            .active_message
            .and_then(|index| self.view.messages.get(index))
        else {
            return Vec::new();
        };
        let mut actions = vec![Action::Reply];
        if message.direction == MessageDirection::Outgoing && message.id.0 > 0 {
            actions.push(Action::Edit);
        }
        if !self.selected_message_ids().is_empty() {
            actions.extend([Action::Delete, Action::Forward]);
        }
        actions.push(Action::React);
        if active_link(message).is_some() {
            actions.push(Action::OpenLink);
        }
        if message
            .details
            .media
            .as_ref()
            .is_some_and(|media| media.remote_id.is_some())
        {
            actions.extend([Action::DownloadMedia, Action::SaveAs]);
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
            Some(download.chat) == self.active_chat_id() && download.message == message.id
        }) {
            actions.push(Action::OpenDownload);
        }
        actions.push(Action::OpenThread);
        if message.id.0 > 0
            && self
                .view
                .active_chat
                .and_then(|index| self.view.chats.get(index))
                .is_some_and(|chat| chat.can_pin_messages)
        {
            actions.push(Action::TogglePin);
        }
        actions.push(Action::ToggleMessageSelection);
        actions
    }

    pub(super) fn open_action_menu(&mut self) {
        let items = match self.view.focus {
            Focus::Transcript => self
                .available_message_actions()
                .into_iter()
                .map(|action| ActionMenuItemView {
                    action,
                    label: message_action_label(action).to_owned(),
                })
                .collect::<Vec<_>>(),
            Focus::Chats | Focus::Composer | Focus::Search => Vec::new(),
        };
        if !items.is_empty() {
            self.view.action_menu = Some(ActionMenuView {
                title: "Message Actions".to_owned(),
                selected: 0,
                items,
            });
        }
    }

    pub(super) fn apply_action_menu(&mut self, action: Action) -> Option<Effect> {
        match action {
            Action::MoveUp => {
                let menu = self.view.action_menu.as_mut()?;
                menu.selected = menu.selected.saturating_sub(1);
                None
            }
            Action::MoveDown => {
                let menu = self.view.action_menu.as_mut()?;
                menu.selected = (menu.selected + 1).min(menu.items.len().saturating_sub(1));
                None
            }
            Action::ChooseAction => {
                let menu = self.view.action_menu.take()?;
                let chosen = menu.items.get(menu.selected)?.action;
                self.apply_action(chosen)
            }
            Action::Cancel | Action::OpenActions => {
                self.view.action_menu = None;
                None
            }
            _ => None,
        }
    }
}

const fn message_action_label(action: Action) -> &'static str {
    match action {
        Action::Reply => "Reply",
        Action::Edit => "Edit",
        Action::Delete => "Delete",
        Action::Forward => "Forward",
        Action::React => "React",
        Action::OpenLink => "Open Link",
        Action::DownloadMedia => "Download",
        Action::SaveAs => "Save As",
        Action::VotePoll => "Vote",
        Action::OpenDownload => "Open Download",
        Action::OpenThread => "Open Thread",
        Action::TogglePin => "Pin / Unpin",
        Action::ToggleMessageSelection => "Select Message",
        _ => "Action",
    }
}
