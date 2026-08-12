use serde::{Deserialize, Serialize};

use super::shared::{GeoPoint, LibraryKind, PreparedAttachment, TextEntity};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::outbox) struct MessageSend<T> {
    pub(in crate::outbox) local_message_id: i64,
    pub(in crate::outbox) reply_to: Option<i64>,
    pub(in crate::outbox) content: T,
}

impl<T> MessageSend<T> {
    pub(in crate::outbox) const fn new(
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
pub(in crate::outbox) struct TextMessage {
    pub(in crate::outbox) text: String,
    pub(in crate::outbox) entities: Vec<TextEntity>,
    pub(in crate::outbox) link_preview: bool,
    pub(in crate::outbox) attachments: Vec<PreparedAttachment>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::outbox) struct Poll {
    pub(in crate::outbox) question: String,
    pub(in crate::outbox) options: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::outbox) struct LibraryMedia {
    pub(in crate::outbox) kind: LibraryKind,
    pub(in crate::outbox) document_id: i64,
    pub(in crate::outbox) access_hash: i64,
    pub(in crate::outbox) file_reference: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::outbox) struct Contact {
    pub(in crate::outbox) phone: String,
    pub(in crate::outbox) first_name: String,
    pub(in crate::outbox) last_name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::outbox) struct Venue {
    pub(in crate::outbox) point: GeoPoint,
    pub(in crate::outbox) title: String,
    pub(in crate::outbox) address: String,
    pub(in crate::outbox) provider: String,
    pub(in crate::outbox) venue_id: String,
    pub(in crate::outbox) venue_type: String,
}
