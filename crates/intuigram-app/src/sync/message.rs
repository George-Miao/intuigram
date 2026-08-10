//! Stable conversion between application Messages and Account-store records.

use intuigram_lib::{
    ChatId, DeliveryState, MediaCard, MediaKind, MessageDetails, MessageDirection, MessageId,
    MessageView, PollOptionView, PollView, ReactionView, TextEntity, TextEntityKind,
};
use intuigram_store::StoredMessage;
use serde::{Deserialize, Serialize};

use super::message_metadata::{
    StoredSpecializedMedia, cached_specialized_media, stored_specialized_media,
};

#[derive(Default, Deserialize, Serialize)]
#[serde(default)]
struct StoredMessageMetadata {
    date_label: String,
    edited: bool,
    pinned: bool,
    forwarded_from: Option<String>,
    views: Option<u32>,
    forwards: Option<u32>,
    replies: Option<u32>,
    service: Option<String>,
    album_id: Option<i64>,
    media: Option<StoredMediaMetadata>,
    reactions: Vec<StoredReaction>,
    entities: Vec<StoredEntity>,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(default)]
struct StoredMediaMetadata {
    title: String,
    description: String,
    details: Vec<String>,
    poll: Option<StoredPoll>,
    specialized: Option<StoredSpecializedMedia>,
    remote_id: Option<String>,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(default)]
struct StoredPoll {
    quiz: bool,
    multiple_choice: bool,
    closed: bool,
    total_voters: Option<u32>,
    options: Vec<StoredPollOption>,
    solution: Option<String>,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(default)]
struct StoredPollOption {
    text: String,
    voters: Option<u32>,
    chosen: bool,
    correct: bool,
}

#[derive(Deserialize, Serialize)]
struct StoredReaction {
    label: String,
    count: u32,
    chosen: bool,
}

#[derive(Deserialize, Serialize)]
struct StoredEntity {
    offset: usize,
    length: usize,
    kind: String,
    value: Option<String>,
    document_id: Option<i64>,
}

/// Converts one application Message into a normalized durable record.
#[must_use]
pub fn encode_stored_message(chat: ChatId, message: &MessageView) -> StoredMessage {
    let content_kind = message.details.media.as_ref().map_or_else(
        || {
            if message.details.service.is_some() {
                "service".to_owned()
            } else {
                "text".to_owned()
            }
        },
        |media| format!("{:?}", media.kind).to_ascii_lowercase(),
    );
    let metadata = serde_json::to_string(&stored_message_metadata(message))
        .expect("fixed Intuigram Message metadata is always JSON-serializable");
    StoredMessage {
        chat_id: chat.0,
        id: message.id.0,
        sender: message.sender.clone(),
        body: message.body.clone(),
        timestamp: message.timestamp.clone(),
        direction: match message.direction {
            MessageDirection::Incoming => "incoming",
            MessageDirection::Outgoing => "outgoing",
        }
        .to_owned(),
        delivery: match message.delivery {
            DeliveryState::Saving => "saving",
            DeliveryState::Pending => "pending",
            DeliveryState::Sent => "sent",
            DeliveryState::Read => "read",
            DeliveryState::Failed => "failed",
        }
        .to_owned(),
        reply_to: message.reply_to.map(|message| message.0),
        thread_root: message.details.thread_root.map(|message| message.0),
        saved_peer: message.details.saved_peer.map(|peer| peer.0),
        content_kind,
        metadata,
    }
}

/// Restores one normalized durable record into application view data.
#[must_use]
pub fn decode_stored_message(message: StoredMessage) -> MessageView {
    let metadata =
        serde_json::from_str::<StoredMessageMetadata>(&message.metadata).unwrap_or_default();
    let media = stored_media_kind(&message.content_kind).map(|kind| {
        let stored = metadata.media.as_ref();
        MediaCard {
            kind,
            title: stored.map_or_else(|| message.content_kind.clone(), |media| media.title.clone()),
            description: stored.map_or_else(String::new, |media| media.description.clone()),
            details: stored.map_or_else(Vec::new, |media| media.details.clone()),
            poll: stored
                .and_then(|media| media.poll.as_ref())
                .map(cached_poll),
            specialized: stored
                .and_then(|media| media.specialized.as_ref())
                .map(cached_specialized_media),
            remote_id: stored.and_then(|media| media.remote_id.clone()),
        }
    });
    MessageView {
        id: MessageId(message.id),
        sender: message.sender,
        body: message.body,
        timestamp: message.timestamp,
        direction: if message.direction == "outgoing" {
            MessageDirection::Outgoing
        } else {
            MessageDirection::Incoming
        },
        delivery: match message.delivery.as_str() {
            "saving" => DeliveryState::Saving,
            "pending" => DeliveryState::Pending,
            "read" => DeliveryState::Read,
            "failed" => DeliveryState::Failed,
            _ => DeliveryState::Sent,
        },
        reply_to: message.reply_to.map(MessageId),
        details: MessageDetails {
            sender_peer: None,
            date_label: metadata.date_label,
            entities: metadata.entities.into_iter().map(cached_entity).collect(),
            forwarded_from: metadata.forwarded_from,
            reactions: metadata
                .reactions
                .into_iter()
                .map(|reaction| ReactionView {
                    label: reaction.label,
                    count: reaction.count,
                    chosen: reaction.chosen,
                })
                .collect(),
            edited: metadata.edited,
            pinned: metadata.pinned,
            views: metadata.views,
            forwards: metadata.forwards,
            replies: metadata.replies,
            media,
            album_id: metadata.album_id,
            service: metadata.service,
            thread_root: message.thread_root.map(MessageId),
            saved_peer: message.saved_peer.map(ChatId),
        },
    }
}

fn stored_message_metadata(message: &MessageView) -> StoredMessageMetadata {
    StoredMessageMetadata {
        date_label: message.details.date_label.clone(),
        edited: message.details.edited,
        pinned: message.details.pinned,
        forwarded_from: message.details.forwarded_from.clone(),
        views: message.details.views,
        forwards: message.details.forwards,
        replies: message.details.replies,
        service: message.details.service.clone(),
        album_id: message.details.album_id,
        media: message
            .details
            .media
            .as_ref()
            .map(|media| StoredMediaMetadata {
                title: media.title.clone(),
                description: media.description.clone(),
                details: media.details.clone(),
                poll: media.poll.as_ref().map(stored_poll),
                specialized: media.specialized.as_ref().map(stored_specialized_media),
                remote_id: media.remote_id.clone(),
            }),
        reactions: message
            .details
            .reactions
            .iter()
            .map(|reaction| StoredReaction {
                label: reaction.label.clone(),
                count: reaction.count,
                chosen: reaction.chosen,
            })
            .collect(),
        entities: message.details.entities.iter().map(stored_entity).collect(),
    }
}

fn stored_poll(poll: &PollView) -> StoredPoll {
    StoredPoll {
        quiz: poll.quiz,
        multiple_choice: poll.multiple_choice,
        closed: poll.closed,
        total_voters: poll.total_voters,
        options: poll
            .options
            .iter()
            .map(|option| StoredPollOption {
                text: option.text.clone(),
                voters: option.voters,
                chosen: option.chosen,
                correct: option.correct,
            })
            .collect(),
        solution: poll.solution.clone(),
    }
}

fn cached_poll(poll: &StoredPoll) -> PollView {
    PollView {
        quiz: poll.quiz,
        multiple_choice: poll.multiple_choice,
        closed: poll.closed,
        total_voters: poll.total_voters,
        options: poll
            .options
            .iter()
            .map(|option| PollOptionView {
                text: option.text.clone(),
                voters: option.voters,
                chosen: option.chosen,
                correct: option.correct,
            })
            .collect(),
        solution: poll.solution.clone(),
    }
}

fn stored_media_kind(kind: &str) -> Option<MediaKind> {
    Some(match kind {
        "photo" => MediaKind::Photo,
        "video" => MediaKind::Video,
        "animation" => MediaKind::Animation,
        "sticker" => MediaKind::Sticker,
        "file" => MediaKind::File,
        "audio" => MediaKind::Audio,
        "voice" => MediaKind::Voice,
        "videonote" => MediaKind::VideoNote,
        "linkpreview" => MediaKind::LinkPreview,
        "poll" => MediaKind::Poll,
        "contact" => MediaKind::Contact,
        "location" => MediaKind::Location,
        "venue" => MediaKind::Venue,
        "dice" => MediaKind::Dice,
        "livelocation" => MediaKind::LiveLocation,
        "game" => MediaKind::Game,
        "invoice" => MediaKind::Invoice,
        "paidmedia" => MediaKind::PaidMedia,
        "giveaway" => MediaKind::Giveaway,
        "gift" => MediaKind::Gift,
        "story" => MediaKind::Story,
        "todolist" => MediaKind::TodoList,
        "specialized" => MediaKind::Unsupported,
        "unsupported" => MediaKind::Unsupported,
        "text" | "service" => return None,
        _ => MediaKind::Unsupported,
    })
}

fn cached_entity(entity: StoredEntity) -> TextEntity {
    TextEntity {
        offset: entity.offset,
        length: entity.length,
        kind: match entity.kind.as_str() {
            "bold" => TextEntityKind::Bold,
            "italic" => TextEntityKind::Italic,
            "underline" => TextEntityKind::Underline,
            "strike" => TextEntityKind::Strike,
            "code" => TextEntityKind::Code,
            "pre" => TextEntityKind::Pre {
                language: entity.value,
            },
            "spoiler" => TextEntityKind::Spoiler,
            "url" => TextEntityKind::Url,
            "text_url" => TextEntityKind::TextUrl {
                url: entity.value.unwrap_or_default(),
            },
            "custom_emoji" => TextEntityKind::CustomEmoji {
                document_id: entity.document_id.unwrap_or_default(),
            },
            _ => TextEntityKind::Semantic,
        },
    }
}

fn stored_entity(entity: &TextEntity) -> StoredEntity {
    let (kind, value, document_id) = match &entity.kind {
        TextEntityKind::Bold => ("bold", None, None),
        TextEntityKind::Italic => ("italic", None, None),
        TextEntityKind::Underline => ("underline", None, None),
        TextEntityKind::Strike => ("strike", None, None),
        TextEntityKind::Code => ("code", None, None),
        TextEntityKind::Pre { language } => ("pre", language.clone(), None),
        TextEntityKind::Spoiler => ("spoiler", None, None),
        TextEntityKind::Url => ("url", None, None),
        TextEntityKind::TextUrl { url } => ("text_url", Some(url.clone()), None),
        TextEntityKind::Semantic => ("semantic", None, None),
        TextEntityKind::CustomEmoji { document_id } => ("custom_emoji", None, Some(*document_id)),
    };
    StoredEntity {
        offset: entity.offset,
        length: entity.length,
        kind: kind.to_owned(),
        value,
        document_id,
    }
}
