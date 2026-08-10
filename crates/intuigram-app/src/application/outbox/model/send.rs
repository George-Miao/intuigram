use serde::{Deserialize, Serialize};

use super::shared::{GeoPoint, LibraryKind, PreparedAttachment, TextEntity};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::application::outbox) struct MessageSend<T> {
    pub(in crate::application::outbox) local_message_id: i64,
    pub(in crate::application::outbox) reply_to: Option<i64>,
    pub(in crate::application::outbox) content: T,
}

impl<T> MessageSend<T> {
    pub(in crate::application::outbox) const fn new(
        local_message_id: i64,
        reply_to: Option<i64>,
        content: T,
    ) -> Self {
        Self {
            local_message_id,
            reply_to,
            content,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::application::outbox) struct TextMessage {
    pub(in crate::application::outbox) text: String,
    pub(in crate::application::outbox) entities: Vec<TextEntity>,
    pub(in crate::application::outbox) link_preview: bool,
    pub(in crate::application::outbox) attachments: Vec<PreparedAttachment>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::application::outbox) struct Poll {
    pub(in crate::application::outbox) question: String,
    pub(in crate::application::outbox) options: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::application::outbox) struct LibraryMedia {
    pub(in crate::application::outbox) kind: LibraryKind,
    pub(in crate::application::outbox) document_id: i64,
    pub(in crate::application::outbox) access_hash: i64,
    pub(in crate::application::outbox) file_reference: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::application::outbox) struct Contact {
    pub(in crate::application::outbox) phone: String,
    pub(in crate::application::outbox) first_name: String,
    pub(in crate::application::outbox) last_name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::application::outbox) struct Venue {
    pub(in crate::application::outbox) point: GeoPoint,
    pub(in crate::application::outbox) title: String,
    pub(in crate::application::outbox) address: String,
    pub(in crate::application::outbox) provider: String,
    pub(in crate::application::outbox) venue_id: String,
    pub(in crate::application::outbox) venue_type: String,
}
