use std::collections::{HashMap, HashSet};

use intuigram_lib::{MessageId, TopicDraftView, TopicId, TopicView};

use super::*;

const TOPIC_PAGE_SIZE: i32 = 100;

impl Client {
    /// Loads the complete ordered Topic projection for one forum or
    /// topic-enabled bot Chat.
    pub async fn forum_topics(&mut self, chat: ChatId) -> Result<Vec<TopicView>> {
        match self.forum_topics_inner(chat).await {
            Err(error) if error.requires_peer_refresh() => {
                self.refresh_peer_directory().await?;
                self.forum_topics_inner(chat).await
            }
            result => result,
        }
    }

    async fn forum_topics_inner(&mut self, chat: ChatId) -> Result<Vec<TopicView>> {
        let peer = self.peers.resolve(chat)?;
        let mut result = Vec::new();
        let mut seen = HashSet::new();
        let mut offset = (0, 0, 0);
        loop {
            let response = self
                .connection
                .invoke(&tl::functions::messages::GetForumTopics {
                    peer: peer.clone(),
                    q: None,
                    offset_date: offset.0,
                    offset_id: offset.1,
                    offset_topic: offset.2,
                    limit: TOPIC_PAGE_SIZE,
                })
                .await
                .context(InvokeSnafu)?;
            let tl::enums::messages::ForumTopics::Topics(page) = response;
            self.update_peer_cache(&page.chats, &page.users);
            let messages = page
                .messages
                .iter()
                .filter_map(|message| {
                    normalize_message(message, &self.names).map(|message| (message.id, message))
                })
                .collect::<HashMap<_, _>>();
            let next = page.topics.iter().rev().find_map(topic_offset);
            let page_len = page.topics.len();
            for topic in page.topics {
                let tl::enums::ForumTopic::Topic(topic) = topic else {
                    continue;
                };
                let id = TopicId(i64::from(topic.id));
                if seen.insert(id) {
                    let top_message = MessageId(i64::from(topic.top_message));
                    result.push(normalize_topic(topic, messages.get(&top_message)));
                }
            }
            let complete = result.len() >= usize::try_from(page.count.max(0)).unwrap_or(0)
                || page_len < usize::try_from(TOPIC_PAGE_SIZE).unwrap_or(100);
            let Some(next) = next else {
                break;
            };
            if complete || next == offset {
                break;
            }
            offset = next;
        }
        ensure_general(&mut result);
        Ok(result)
    }
}

fn topic_offset(topic: &tl::enums::ForumTopic) -> Option<(i32, i32, i32)> {
    match topic {
        tl::enums::ForumTopic::Topic(topic) => Some((topic.date, topic.top_message, topic.id)),
        tl::enums::ForumTopic::Deleted(_) => None,
    }
}

fn normalize_topic(topic: tl::types::ForumTopic, top: Option<&MessageView>) -> TopicView {
    TopicView {
        id: TopicId(i64::from(topic.id)),
        title: topic.title,
        preview: top.map_or_else(String::new, |message| message.body.clone()),
        timestamp: top.map_or_else(String::new, |message| message.timestamp.clone()),
        unread: u32::try_from(topic.unread_count.max(0)).unwrap_or(0),
        pinned: topic.pinned,
        closed: topic.closed,
        hidden: topic.hidden,
        icon_color: u32::try_from(topic.icon_color).unwrap_or(0),
        icon_emoji_id: topic.icon_emoji_id,
        top_message: (topic.top_message > 0).then_some(MessageId(i64::from(topic.top_message))),
        draft: topic.draft.and_then(normalize_topic_draft),
    }
}

fn normalize_topic_draft(draft: tl::enums::DraftMessage) -> Option<TopicDraftView> {
    let tl::enums::DraftMessage::Message(draft) = draft else {
        return None;
    };
    Some(TopicDraftView {
        text: draft.message,
        reply_to: draft.reply_to.and_then(|reply| match reply {
            tl::enums::InputReplyTo::Message(reply) => {
                Some(MessageId(i64::from(reply.reply_to_msg_id)))
            }
            tl::enums::InputReplyTo::Story(_) | tl::enums::InputReplyTo::MonoForum(_) => None,
        }),
    })
}

fn ensure_general(topics: &mut Vec<TopicView>) {
    if topics.iter().any(|topic| topic.id == TopicId(1)) {
        return;
    }
    topics.insert(
        0,
        TopicView {
            id: TopicId(1),
            title: "General".to_owned(),
            preview: String::new(),
            timestamp: String::new(),
            unread: 0,
            pinned: false,
            closed: false,
            hidden: true,
            icon_color: 0x6f_76_5b,
            icon_emoji_id: None,
            top_message: None,
            draft: None,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_general_is_synthesized_when_a_page_omits_it() {
        let mut topics = vec![TopicView {
            id: TopicId(40),
            title: "Design".to_owned(),
            preview: String::new(),
            timestamp: String::new(),
            unread: 0,
            pinned: false,
            closed: false,
            hidden: false,
            icon_color: 0,
            icon_emoji_id: None,
            top_message: None,
            draft: None,
        }];

        ensure_general(&mut topics);

        assert_eq!(topics[0].id, TopicId(1));
        assert_eq!(topics[0].title, "General");
        assert!(topics[0].hidden);
    }
}
