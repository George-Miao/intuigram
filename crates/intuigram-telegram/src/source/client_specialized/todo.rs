use super::*;

impl Client {
    /// Toggles one TODO item and returns the authoritative normalized list.
    pub async fn toggle_todo_item(
        &mut self,
        chat: ChatId,
        message: MessageId,
        item: i32,
        completed: bool,
    ) -> Result<MediaCard> {
        let media = self.message_media(chat, message).await?;
        let tl::enums::MessageMedia::ToDo(todo) = &media else {
            return SpecializedMediaUnavailableSnafu {
                message_id: message.0,
                family: "a TODO list",
            }
            .fail();
        };
        let tl::enums::TodoList::List(list) = &todo.todo;
        if !list.list.iter().any(|candidate| {
            let tl::enums::TodoItem::Item(candidate) = candidate;
            candidate.id == item
        }) {
            return TodoItemUnavailableSnafu {
                message_id: message.0,
                item,
            }
            .fail();
        }
        let peer = self.peers.resolve(chat)?;
        let msg_id = telegram_message_id(message)?;
        self.connection
            .invoke(&tl::functions::messages::ToggleTodoCompleted {
                peer,
                msg_id,
                completed: completed.then_some(vec![item]).unwrap_or_default(),
                incompleted: (!completed).then_some(vec![item]).unwrap_or_default(),
            })
            .await
            .context(InvokeSnafu)?;
        self.refreshed_family(chat, message, "a TODO list", |media| {
            matches!(media, tl::enums::MessageMedia::ToDo(_))
        })
        .await
    }

    /// Appends one plain-text TODO item and returns the authoritative list.
    pub async fn append_todo_item(
        &mut self,
        chat: ChatId,
        message: MessageId,
        title: String,
    ) -> Result<MediaCard> {
        let media = self.message_media(chat, message).await?;
        let tl::enums::MessageMedia::ToDo(todo) = &media else {
            return SpecializedMediaUnavailableSnafu {
                message_id: message.0,
                family: "a TODO list",
            }
            .fail();
        };
        let tl::enums::TodoList::List(list) = &todo.todo;
        let id = list
            .list
            .iter()
            .map(|item| {
                let tl::enums::TodoItem::Item(item) = item;
                item.id
            })
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let peer = self.peers.resolve(chat)?;
        let msg_id = telegram_message_id(message)?;
        self.connection
            .invoke(&tl::functions::messages::AppendTodoList {
                peer,
                msg_id,
                list: vec![
                    tl::types::TodoItem {
                        id,
                        title: tl::types::TextWithEntities {
                            text: title,
                            entities: Vec::new(),
                        }
                        .into(),
                    }
                    .into(),
                ],
            })
            .await
            .context(InvokeSnafu)?;
        self.refreshed_family(chat, message, "a TODO list", |media| {
            matches!(media, tl::enums::MessageMedia::ToDo(_))
        })
        .await
    }
}
