use super::*;

impl Backend {
    pub(super) async fn load_topics(&mut self, chat: ChatId) -> Result<Option<AdapterEvent>> {
        match self.client.forum_topics(chat).await {
            Ok(topics) => {
                self.store
                    .save_topics(
                        chat.0,
                        topics
                            .iter()
                            .map(|topic| stored_topic(chat, topic))
                            .collect(),
                    )
                    .context(AccountDatabaseSnafu)?
                    .await
                    .context(AccountDatabaseSnafu)?;
                Ok(Some(AdapterEvent::TopicsLoaded(TopicListView {
                    chat,
                    topics,
                })))
            }
            Err(source) if source.is_connection_failure() => Err(Error::Telegram { source }),
            Err(error) => Ok(Some(AdapterEvent::TopicsLoadFailed(TopicLoadFailure {
                chat,
                reason: error.to_string(),
            }))),
        }
    }
}

pub(super) fn stored_topic(chat: ChatId, topic: &TopicView) -> StoredTopic {
    StoredTopic {
        chat_id: chat.0,
        id: topic.id.0,
        title: topic.title.clone(),
        preview: topic.preview.clone(),
        timestamp: topic.timestamp.clone(),
        unread: topic.unread,
        pinned: topic.pinned,
        closed: topic.closed,
        hidden: topic.hidden,
        icon_color: topic.icon_color,
        icon_emoji_id: topic.icon_emoji_id,
        top_message_id: topic.top_message.map(|message| message.0),
        draft_text: topic.draft.as_ref().map(|draft| draft.text.clone()),
        draft_reply_to: topic
            .draft
            .as_ref()
            .and_then(|draft| draft.reply_to)
            .map(|message| message.0),
    }
}
