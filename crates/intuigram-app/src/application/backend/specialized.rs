use super::*;

impl Backend {
    pub(super) async fn execute_specialized(
        &mut self,
        effect: Effect,
    ) -> Result<Option<AdapterEvent>> {
        match effect {
            Effect::RefreshSpecialized {
                chat,
                message,
                target,
            } => self.refresh_specialized(chat, *message, target).await,
            Effect::ToggleTodoItem {
                chat,
                message,
                item,
                completed,
            } => self.toggle_todo_item(chat, *message, item, completed).await,
            Effect::AppendTodoItem {
                chat,
                message,
                title,
            } => self.append_todo_item(chat, *message, title).await,
            _ => unreachable!("the specialized-effect route accepts only specialized effects"),
        }
    }

    pub(super) async fn refresh_specialized(
        &mut self,
        chat: ChatId,
        mut message: MessageView,
        target: SpecializedRefreshTarget,
    ) -> Result<Option<AdapterEvent>> {
        let result = self
            .client
            .refresh_specialized(chat, message.id, target)
            .await;
        self.commit_specialized_result(chat, &mut message, result)
            .await
    }

    pub(super) async fn toggle_todo_item(
        &mut self,
        chat: ChatId,
        mut message: MessageView,
        item: i32,
        completed: bool,
    ) -> Result<Option<AdapterEvent>> {
        let result = self
            .client
            .toggle_todo_item(chat, message.id, item, completed)
            .await;
        self.commit_specialized_result(chat, &mut message, result)
            .await
    }

    pub(super) async fn append_todo_item(
        &mut self,
        chat: ChatId,
        mut message: MessageView,
        title: String,
    ) -> Result<Option<AdapterEvent>> {
        let result = self.client.append_todo_item(chat, message.id, title).await;
        self.commit_specialized_result(chat, &mut message, result)
            .await
    }

    async fn commit_specialized_result(
        &mut self,
        chat: ChatId,
        message: &mut MessageView,
        result: intuigram_telegram::Result<MediaCard>,
    ) -> Result<Option<AdapterEvent>> {
        match result {
            Ok(mut media) => {
                if let (
                    Some(SpecializedMediaView::Story(previous)),
                    Some(SpecializedMediaView::Story(refreshed)),
                ) = (
                    message
                        .details
                        .media
                        .as_ref()
                        .and_then(|media| media.specialized.as_ref()),
                    media.specialized.as_mut(),
                ) {
                    refreshed.via_mention = previous.via_mention;
                }
                message.details.media = Some(media);
                self.store
                    .save_messages(vec![encode_stored_message(chat, message)])
                    .context(AccountDatabaseSnafu)?
                    .await
                    .context(AccountDatabaseSnafu)?;
                Ok(Some(AdapterEvent::MessageUpdated {
                    chat,
                    message: Box::new(message.clone()),
                }))
            }
            Err(source) if source.is_connection_failure() => Err(Error::Telegram { source }),
            Err(error) => Ok(Some(AdapterEvent::OperationFailed(error.to_string()))),
        }
    }
}
