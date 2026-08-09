use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub(in crate::application::outbox) struct MediaPosition(pub(in crate::application::outbox) u32);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::application::outbox) enum AttachmentKind {
    Photo,
    Video,
    File,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::application::outbox) enum UploadKind {
    Photo,
    Video,
    File,
    Animation,
    Sticker,
    CustomEmoji,
    Voice,
    VideoNote,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::application::outbox) enum LibraryKind {
    Stickers,
    Gifs,
    CustomEmoji,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::application::outbox) struct PreparedAttachment {
    pub(in crate::application::outbox) position: MediaPosition,
    pub(in crate::application::outbox) kind: AttachmentKind,
}

impl PreparedAttachment {
    pub(in crate::application::outbox) const fn new(
        position: MediaPosition,
        kind: AttachmentKind,
    ) -> Self {
        Self { position, kind }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::application::outbox) struct PreparedMedia {
    pub(in crate::application::outbox) position: MediaPosition,
    pub(in crate::application::outbox) kind: UploadKind,
}

impl PreparedMedia {
    pub(in crate::application::outbox) const fn new(
        position: MediaPosition,
        kind: UploadKind,
    ) -> Self {
        Self { position, kind }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::application::outbox) struct GeoPoint {
    pub(in crate::application::outbox) latitude_microdegrees: i32,
    pub(in crate::application::outbox) longitude_microdegrees: i32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    rename_all = "snake_case",
    tag = "kind",
    content = "data"
)]
pub(in crate::application::outbox) enum TextEntityKind {
    Bold,

    Italic,

    Underline,

    Strike,

    Code,

    Pre { language: Option<String> },

    Spoiler,

    Url,

    TextUrl { url: String },

    Semantic,

    CustomEmoji { document_id: i64 },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::application::outbox) struct TextEntity {
    pub(in crate::application::outbox) offset: u32,
    pub(in crate::application::outbox) length: u32,
    pub(in crate::application::outbox) kind: TextEntityKind,
}
