use super::*;

impl Backend {
    pub(super) async fn edit_message(
        &mut self,
        chat: ChatId,
        message: MessageView,
        draft_text: String,
        attachment_ids: Vec<AttachmentId>,
        draft_attachments: Vec<AttachmentView>,
        random_id: i64,
    ) -> Result<Option<AdapterEvent>> {
        let upload = attachment_ids
            .last()
            .map(|id| {
                self.attachments
                    .payloads
                    .get(id)
                    .ok_or(Error::MissingPreparedAttachment { attachment: *id })
                    .and_then(super::attachments::prepared_upload)
                    .map(|upload| (upload, derived_random_id(random_id, 0, 0x4544_4954)))
            })
            .transpose()?;
        let result = self
            .client
            .edit_message(intuigram_telegram::MessageEdit {
                chat,
                message: message.id,
                text: message.body.clone(),
                entities: message.details.entities.clone(),
                upload,
            })
            .await;
        match result {
            Ok(()) => {
                self.store
                    .save_messages(vec![encode_stored_message(chat, &message)])
                    .context(AccountDatabaseSnafu)?
                    .await
                    .context(AccountDatabaseSnafu)?;
                for id in attachment_ids {
                    self.attachments.payloads.remove(&id);
                }
                Ok(Some(AdapterEvent::MessageUpdated {
                    chat,
                    message: Box::new(message),
                }))
            }
            Err(source) if source.is_connection_failure() => Err(Error::Telegram { source }),
            Err(error) => Ok(Some(AdapterEvent::MessageEditFailed {
                chat,
                message: message.id,
                text: draft_text,
                attachments: draft_attachments,
                reason: error.to_string(),
            })),
        }
    }

    pub(super) async fn delete_messages(
        &mut self,
        chat: ChatId,
        messages: Vec<MessageId>,
    ) -> Result<Option<AdapterEvent>> {
        let result = self.client.delete_messages(chat, messages.clone()).await;
        match result {
            Ok(()) => {
                self.store
                    .delete_messages(
                        Some(chat.0),
                        messages.iter().map(|message| message.0).collect(),
                    )
                    .context(AccountDatabaseSnafu)?
                    .await
                    .context(AccountDatabaseSnafu)?;
                Ok(Some(AdapterEvent::MessagesDeleted {
                    chat: Some(chat),
                    ids: messages,
                }))
            }
            Err(source) if source.is_connection_failure() => Err(Error::Telegram { source }),
            Err(error) => Ok(Some(AdapterEvent::OperationFailed(error.to_string()))),
        }
    }

    pub(super) async fn forward_messages(
        &mut self,
        source: ChatId,
        destination: ChatId,
        destination_saved_peer: Option<ChatId>,
        messages: Vec<MessageId>,
        random_id: i64,
    ) -> Result<Option<AdapterEvent>> {
        match self
            .client
            .forward_messages(
                source,
                destination,
                destination_saved_peer,
                messages,
                random_id,
            )
            .await
        {
            Ok(()) => Ok(None),
            Err(source) if source.is_connection_failure() => Err(Error::Telegram { source }),
            Err(error) => Ok(Some(AdapterEvent::OperationFailed(error.to_string()))),
        }
    }

    pub(super) async fn react_message(
        &mut self,
        chat: ChatId,
        message: MessageView,
        reaction: String,
    ) -> Result<Option<AdapterEvent>> {
        match self.client.react_message(chat, message.id, reaction).await {
            Ok(()) => {
                self.store
                    .save_messages(vec![encode_stored_message(chat, &message)])
                    .context(AccountDatabaseSnafu)?
                    .await
                    .context(AccountDatabaseSnafu)?;
                Ok(Some(AdapterEvent::MessageUpdated {
                    chat,
                    message: Box::new(message),
                }))
            }
            Err(source) if source.is_connection_failure() => Err(Error::Telegram { source }),
            Err(error) => Ok(Some(AdapterEvent::OperationFailed(error.to_string()))),
        }
    }

    pub(super) async fn vote_poll(
        &mut self,
        chat: ChatId,
        mut message: MessageView,
        options: Vec<usize>,
    ) -> Result<Option<AdapterEvent>> {
        match self.client.vote_poll(chat, message.id, options).await {
            Ok(media) => {
                message.details.media = Some(media);
                self.store
                    .save_messages(vec![encode_stored_message(chat, &message)])
                    .context(AccountDatabaseSnafu)?
                    .await
                    .context(AccountDatabaseSnafu)?;
                Ok(Some(AdapterEvent::MessageUpdated {
                    chat,
                    message: Box::new(message),
                }))
            }
            Err(source) if source.is_connection_failure() => Err(Error::Telegram { source }),
            Err(error) => Ok(Some(AdapterEvent::OperationFailed(error.to_string()))),
        }
    }
}
