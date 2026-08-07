impl App {
    pub(super) fn begin_previous_edit(&mut self) {
        let Some(index) = self.view.messages.iter().rposition(|message| {
            message.direction == MessageDirection::Outgoing && message.id.0 > 0
        }) else {
            return;
        };
        self.view.active_message = Some(index);
        self.begin_edit();
    }

    pub(super) fn open_reaction_picker(&mut self) {
        if self.active_message_id().is_some() {
            self.view.reaction_picker = Some(ReactionPickerView {
                selected: 0,
                options: ["👍", "❤️", "🔥", "👏", "😁", "🤔"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
            });
        }
    }

    pub(super) fn apply_reaction_picker(&mut self, action: Action) -> Option<Effect> {
        match action {
            Action::MoveUp | Action::MoveDown => {
                let picker = self.view.reaction_picker.as_mut()?;
                picker.selected = move_index(
                    Some(picker.selected),
                    picker.options.len(),
                    action == Action::MoveDown,
                )?;
                None
            }
            Action::ConfirmReaction => {
                let chat = self.active_chat_id()?;
                let message_id = self.active_message_id()?;
                let picker = self.view.reaction_picker.take()?;
                let reaction = picker.options.get(picker.selected)?.clone();
                let mut message = self
                    .view
                    .messages
                    .iter()
                    .find(|message| message.id == message_id)?
                    .clone();
                let already_chosen = message
                    .details
                    .reactions
                    .iter()
                    .any(|existing| existing.label == reaction && existing.chosen);
                for existing in &mut message.details.reactions {
                    existing.chosen = false;
                }
                if let Some(existing) = message
                    .details
                    .reactions
                    .iter_mut()
                    .find(|existing| existing.label == reaction)
                {
                    existing.count = existing.count.saturating_add(u32::from(!already_chosen));
                    existing.chosen = true;
                } else {
                    message.details.reactions.push(ReactionView {
                        label: reaction.clone(),
                        count: 1,
                        chosen: true,
                    });
                }
                Some(Effect::ReactMessage {
                    chat,
                    message: Box::new(message),
                    reaction,
                })
            }
            Action::Cancel => {
                self.view.reaction_picker = None;
                None
            }
            _ => None,
        }
    }

    pub(super) fn open_forward_picker(&mut self) {
        if self.selected_message_ids().is_empty() {
            return;
        }
        let Some(source) = self.active_chat_id() else {
            return;
        };
        self.view.forward_picker = self
            .view
            .chats
            .iter()
            .position(|candidate| candidate.id != source);
    }

    pub(super) fn apply_forward_picker(&mut self, action: Action) -> Option<Effect> {
        let source = self.active_chat_id()?;
        match action {
            Action::MoveUp | Action::MoveDown => {
                let forward = action == Action::MoveDown;
                let current = self.view.forward_picker?;
                let candidates = (0..self.view.chats.len())
                    .filter(|index| self.view.chats[*index].id != source)
                    .collect::<Vec<_>>();
                let position = candidates
                    .iter()
                    .position(|candidate| *candidate == current)
                    .unwrap_or(0);
                let next = move_index(Some(position), candidates.len(), forward)?;
                self.view.forward_picker = candidates.get(next).copied();
                None
            }
            Action::ConfirmForward => {
                let destination = self
                    .view
                    .forward_picker
                    .and_then(|index| self.view.chats.get(index))?
                    .id;
                let messages = self.selected_message_ids();
                if messages.is_empty() {
                    return None;
                }
                self.view.forward_picker = None;
                self.clear_message_selection();
                Some(Effect::ForwardMessages {
                    source,
                    destination,
                    messages,
                })
            }
            Action::Cancel => {
                self.view.forward_picker = None;
                None
            }
            _ => None,
        }
    }

    pub(super) fn apply_delete_confirmation(&mut self, action: Action) -> Option<Effect> {
        match action {
            Action::ConfirmDelete => {
                let chat = self.active_chat_id()?;
                let messages = self.view.delete_confirmation.take()?;
                self.clear_message_selection();
                Some(Effect::DeleteMessages { chat, messages })
            }
            Action::Cancel => {
                self.view.delete_confirmation = None;
                None
            }
            _ => None,
        }
    }

    pub(super) fn begin_edit(&mut self) {
        let Some(message) = self
            .view
            .active_message
            .and_then(|index| self.view.messages.get(index))
            .filter(|message| message.direction == MessageDirection::Outgoing && message.id.0 > 0)
            .cloned()
        else {
            return;
        };
        self.view.transcript_anchor = self.view.active_message;
        self.view.active_message = None;
        self.view.composer = ComposerView {
            cursor: message.body.len(),
            text: message.body,
            editing: Some(message.id),
            ..ComposerView::default()
        };
        self.view.focus = Focus::Composer;
    }

    pub(super) fn save_edit(&mut self) -> Option<Effect> {
        let chat = self.active_chat_id()?;
        let message_id = self.view.composer.editing?;
        let draft_text = self.view.composer.text.trim_end().to_owned();
        if draft_text.is_empty() {
            return None;
        }
        let formatted = format_markdown(&draft_text);
        let mut message = self
            .view
            .messages
            .iter()
            .find(|message| message.id == message_id)?
            .clone();
        message.body = formatted.text;
        message.details.entities = formatted.entities;
        message.details.edited = true;
        self.finish_edit(message_id);
        Some(Effect::EditMessage {
            chat,
            message: Box::new(message),
            draft_text,
        })
    }

    pub(super) fn cancel_edit(&mut self) {
        if let Some(message) = self.view.composer.editing {
            self.finish_edit(message);
        }
    }

    pub(super) fn finish_edit(&mut self, message: MessageId) {
        self.restore_active_draft();
        self.refresh_active_history_at(Some(message), Some(message));
        self.view.focus = Focus::Transcript;
    }

    pub(super) fn restore_failed_edit(&mut self, chat: ChatId, message: MessageId, text: String) {
        if self.active_chat_id() != Some(chat) {
            return;
        }
        self.view.transcript_anchor = self.history_position(message);
        self.view.active_message = None;
        self.view.composer = ComposerView {
            cursor: text.len(),
            text,
            editing: Some(message),
            ..ComposerView::default()
        };
        self.view.focus = Focus::Composer;
    }
}
use super::*;
