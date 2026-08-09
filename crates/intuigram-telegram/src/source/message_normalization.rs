pub(super) fn text_with_entities(text: tl::enums::TextWithEntities) -> String {
    let tl::enums::TextWithEntities::Entities(text) = text;
    text.text
}

pub(super) fn message_parts(
    messages: tl::enums::messages::Messages,
) -> (
    Vec<tl::enums::Message>,
    Vec<tl::enums::Chat>,
    Vec<tl::enums::User>,
) {
    match messages {
        tl::enums::messages::Messages::Messages(messages) => {
            (messages.messages, messages.chats, messages.users)
        }
        tl::enums::messages::Messages::Slice(messages) => {
            (messages.messages, messages.chats, messages.users)
        }
        tl::enums::messages::Messages::ChannelMessages(messages) => {
            (messages.messages, messages.chats, messages.users)
        }
        tl::enums::messages::Messages::NotModified(_) => (Vec::new(), Vec::new(), Vec::new()),
    }
}

pub(super) const fn mark_channel_id(id: i64) -> i64 {
    -1_000_000_000_000 - id
}

pub(super) const fn marked_peer_id(peer: &tl::enums::Peer) -> ChatId {
    match peer {
        tl::enums::Peer::User(peer) => ChatId(peer.user_id),
        tl::enums::Peer::Chat(peer) => ChatId(-peer.chat_id),
        tl::enums::Peer::Channel(peer) => ChatId(mark_channel_id(peer.channel_id)),
    }
}

pub(super) fn message_chat_id(message: &tl::enums::Message) -> ChatId {
    match message {
        tl::enums::Message::Empty(_) => ChatId(0),
        tl::enums::Message::Message(message) => marked_peer_id(&message.peer_id),
        tl::enums::Message::Service(message) => marked_peer_id(&message.peer_id),
    }
}

pub(super) fn dialog_message_summary(
    message: &tl::enums::Message,
    names: &HashMap<ChatId, String>,
) -> (String, Option<String>, Option<ChatId>, String) {
    let (outgoing, sender, date) = match message {
        tl::enums::Message::Empty(_) => {
            return (String::new(), None, None, String::new());
        }
        tl::enums::Message::Message(message) => {
            (message.out, message.from_id.as_ref(), message.date)
        }
        tl::enums::Message::Service(message) => {
            (message.out, message.from_id.as_ref(), message.date)
        }
    };
    let sender_peer = sender.map(marked_peer_id);
    let sender = if outgoing {
        Some("You".to_owned())
    } else {
        sender_peer.and_then(|id| names.get(&id).cloned())
    };
    (
        message_body(message),
        sender,
        sender_peer,
        format_timestamp(date),
    )
}

pub(super) fn normalize_message(
    message: &tl::enums::Message,
    names: &HashMap<ChatId, String>,
) -> Option<MessageView> {
    match message {
        tl::enums::Message::Empty(_) => None,
        tl::enums::Message::Message(message) => {
            let sender_id = message.from_id.as_ref().map(marked_peer_id);
            let sender = if message.out {
                "You".to_owned()
            } else {
                sender_id
                    .and_then(|id| names.get(&id).cloned())
                    .unwrap_or_else(|| "Unknown sender".to_owned())
            };
            let reply_to = message.reply_to.as_ref().and_then(reply_message_id);
            let media = message.media.as_ref().map(normalize_media);
            let body = if message.message.is_empty() {
                media
                    .as_ref()
                    .map_or_else(|| "[Unsupported content]".to_owned(), media_card_fallback)
            } else {
                message.message.clone()
            };
            Some(MessageView {
                id: MessageId(i64::from(message.id)),
                sender,
                body,
                timestamp: format_timestamp(message.date),
                direction: if message.out {
                    MessageDirection::Outgoing
                } else {
                    MessageDirection::Incoming
                },
                delivery: DeliveryState::Sent,
                reply_to,
                details: MessageDetails {
                    sender_peer: sender_id,
                    date_label: format_date(message.date),
                    entities: normalize_entities(message.entities.as_deref()),
                    forwarded_from: normalize_forward(message.fwd_from.as_ref(), names),
                    reactions: normalize_reactions(message.reactions.as_ref()),
                    edited: message.edit_date.is_some(),
                    pinned: message.pinned,
                    views: nonnegative_u32(message.views),
                    forwards: nonnegative_u32(message.forwards),
                    replies: message.replies.as_ref().and_then(|replies| match replies {
                        tl::enums::MessageReplies::Replies(replies) => {
                            u32::try_from(replies.replies).ok()
                        }
                    }),
                    media,
                    album_id: message.grouped_id,
                    service: None,
                    thread_root: message.reply_to.as_ref().and_then(thread_root_message_id),
                    saved_peer: message.saved_peer_id.as_ref().map(marked_peer_id),
                },
            })
        }
        tl::enums::Message::Service(message) => {
            let description = service_event_description(&message.action);
            let sender_peer = message.from_id.as_ref().map(marked_peer_id);
            Some(MessageView {
                id: MessageId(i64::from(message.id)),
                sender: message
                    .from_id
                    .as_ref()
                    .map(marked_peer_id)
                    .and_then(|id| names.get(&id).cloned())
                    .unwrap_or_else(|| "Telegram".to_owned()),
                body: description.clone(),
                timestamp: format_timestamp(message.date),
                direction: if message.out {
                    MessageDirection::Outgoing
                } else {
                    MessageDirection::Incoming
                },
                delivery: DeliveryState::Sent,
                reply_to: message.reply_to.as_ref().and_then(reply_message_id),
                details: MessageDetails {
                    sender_peer,
                    date_label: format_date(message.date),
                    service: Some(description),
                    saved_peer: message.saved_peer_id.as_ref().map(marked_peer_id),
                    ..MessageDetails::default()
                },
            })
        }
    }
}

pub(super) fn reply_message_id(header: &tl::enums::MessageReplyHeader) -> Option<MessageId> {
    match header {
        tl::enums::MessageReplyHeader::Header(header) => {
            header.reply_to_msg_id.map(|id| MessageId(i64::from(id)))
        }
        tl::enums::MessageReplyHeader::MessageReplyStoryHeader(_) => None,
    }
}

pub(crate) fn thread_root_message_id(header: &tl::enums::MessageReplyHeader) -> Option<MessageId> {
    match header {
        tl::enums::MessageReplyHeader::Header(header) => header
            .reply_to_top_id
            .or_else(|| {
                header
                    .forum_topic
                    .then_some(header.reply_to_msg_id)
                    .flatten()
            })
            .map(|id| MessageId(i64::from(id))),
        tl::enums::MessageReplyHeader::MessageReplyStoryHeader(_) => None,
    }
}

pub(super) fn message_body(message: &tl::enums::Message) -> String {
    match message {
        tl::enums::Message::Message(message) if !message.message.is_empty() => {
            message.message.clone()
        }
        tl::enums::Message::Message(message) if message.media.is_some() => message
            .media
            .as_ref()
            .map(normalize_media)
            .as_ref()
            .map_or_else(|| "[Unsupported content]".to_owned(), media_card_fallback),
        tl::enums::Message::Empty(_) | tl::enums::Message::Message(_) => {
            "[Unsupported content]".to_owned()
        }
        tl::enums::Message::Service(_) => "[Service event]".to_owned(),
    }
}

pub(super) fn normalize_entities(entities: Option<&[tl::enums::MessageEntity]>) -> Vec<TextEntity> {
    entities
        .unwrap_or_default()
        .iter()
        .map(|entity| TextEntity {
            offset: usize::try_from(entity.offset()).unwrap_or(0),
            length: usize::try_from(entity.length()).unwrap_or(0),
            kind: match entity {
                tl::enums::MessageEntity::Bold(_) => TextEntityKind::Bold,
                tl::enums::MessageEntity::Italic(_) => TextEntityKind::Italic,
                tl::enums::MessageEntity::Underline(_) => TextEntityKind::Underline,
                tl::enums::MessageEntity::Strike(_) => TextEntityKind::Strike,
                tl::enums::MessageEntity::Code(_) => TextEntityKind::Code,
                tl::enums::MessageEntity::Pre(entity) => TextEntityKind::Pre {
                    language: (!entity.language.is_empty()).then(|| entity.language.clone()),
                },
                tl::enums::MessageEntity::Spoiler(_) => TextEntityKind::Spoiler,
                tl::enums::MessageEntity::Url(_) => TextEntityKind::Url,
                tl::enums::MessageEntity::TextUrl(entity) => TextEntityKind::TextUrl {
                    url: entity.url.clone(),
                },
                tl::enums::MessageEntity::CustomEmoji(entity) => TextEntityKind::CustomEmoji {
                    document_id: entity.document_id,
                },
                _ => TextEntityKind::Semantic,
            },
        })
        .collect()
}
use super::*;
