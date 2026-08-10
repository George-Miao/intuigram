use super::*;

impl App {
    pub(super) fn apply_paid_media_items(
        &mut self,
        chat: ChatId,
        message: MessageId,
        items: Vec<PaidMediaItemView>,
    ) -> Option<Effect> {
        for history in self
            .histories
            .iter_mut()
            .filter(|(key, _)| key.chat == chat)
            .map(|(_, history)| history)
        {
            if let Some(candidate) = history.iter_mut().find(|candidate| candidate.id == message) {
                replace_paid_items(candidate, &items);
            }
        }
        if self.active_chat_id() == Some(chat) {
            self.refresh_active_history();
        }
        None
    }

    pub(super) fn refresh_specialized(&self) -> Option<Effect> {
        let chat = self.active_chat_id()?;
        let message = self
            .view
            .active_message
            .and_then(|index| self.view.messages.get(index))?
            .clone();
        let target = match message.details.media.as_ref()?.specialized.as_ref()? {
            SpecializedMediaView::PaidMedia(media)
                if media
                    .items
                    .iter()
                    .any(|item| matches!(item, PaidMediaItemView::Preview { .. })) =>
            {
                SpecializedRefreshTarget::PaidMedia
            }
            SpecializedMediaView::Story(story) => SpecializedRefreshTarget::Story {
                peer: story.peer,
                id: story.id,
            },
            SpecializedMediaView::Giveaway(_) => SpecializedRefreshTarget::Giveaway,
            _ => return None,
        };
        Some(Effect::RefreshSpecialized {
            chat,
            message: Box::new(message),
            target,
        })
    }

    pub(super) fn open_todo_editor(&mut self) {
        let Some(message) = self
            .view
            .active_message
            .and_then(|index| self.view.messages.get(index))
        else {
            return;
        };
        let Some(SpecializedMediaView::TodoList(todo)) = message
            .details
            .media
            .as_ref()
            .and_then(|media| media.specialized.as_ref())
        else {
            return;
        };
        let owns_message = message.direction == MessageDirection::Outgoing;
        self.view.todo_editor = Some(TodoListEditorView {
            message: message.id,
            selected: 0,
            items: todo.items.clone(),
            append: None,
            can_append: owns_message || todo.others_can_append,
            can_complete: owns_message || todo.others_can_complete,
        });
    }

    pub(super) fn apply_todo_editor(&mut self, action: Action) -> Option<Effect> {
        match action {
            Action::MoveUp | Action::MoveDown => {
                let editor = self.view.todo_editor.as_mut()?;
                editor.selected = move_index(
                    Some(editor.selected),
                    editor.items.len(),
                    action == Action::MoveDown,
                )?;
                None
            }
            Action::ToggleTodoItem => self.toggle_todo_item(),
            Action::AppendTodoItem => {
                let editor = self.view.todo_editor.as_mut()?;
                if editor.can_append {
                    editor.append = Some(String::new());
                }
                None
            }
            Action::ConfirmTodoAppend => self.append_todo_item(),
            Action::Cancel | Action::EditTodoList => {
                let editor = self.view.todo_editor.as_mut()?;
                if editor.append.take().is_none() {
                    self.view.todo_editor = None;
                }
                None
            }
            _ => None,
        }
    }

    fn toggle_todo_item(&mut self) -> Option<Effect> {
        let chat = self.active_chat_id()?;
        let editor = self.view.todo_editor.take()?;
        if !editor.can_complete {
            return None;
        }
        let item = editor.items.get(editor.selected)?;
        let message = self
            .view
            .messages
            .iter()
            .find(|message| message.id == editor.message)?
            .clone();
        Some(Effect::ToggleTodoItem {
            chat,
            message: Box::new(message),
            item: item.id,
            completed: !item.completed,
        })
    }

    fn append_todo_item(&mut self) -> Option<Effect> {
        let chat = self.active_chat_id()?;
        let editor = self.view.todo_editor.take()?;
        let title = editor.append?.trim().to_owned();
        if title.is_empty() || !editor.can_append {
            return None;
        }
        let message = self
            .view
            .messages
            .iter()
            .find(|message| message.id == editor.message)?
            .clone();
        Some(Effect::AppendTodoItem {
            chat,
            message: Box::new(message),
            title,
        })
    }
}

fn replace_paid_items(message: &mut MessageView, items: &[PaidMediaItemView]) {
    if let Some(SpecializedMediaView::PaidMedia(media)) = message
        .details
        .media
        .as_mut()
        .and_then(|media| media.specialized.as_mut())
    {
        media.items = items.to_vec();
    }
}
