use super::super::*;

/// Telegram-owned media library queried for composer selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaLibraryKind {
    /// Recently used stickers.
    Stickers,
    /// Saved animated GIF documents.
    Gifs,
    /// Custom emoji matching an emoji or keyword query.
    CustomEmoji,
}

/// One sendable entry from a Telegram-owned media library.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaLibraryEntry {
    /// Stable Telegram document identifier.
    pub id: i64,

    /// Human-readable emoji, filename, or MIME fallback.
    pub label: String,

    pub(super) kind: MediaLibraryKind,

    pub(super) access_hash: i64,

    pub(super) file_reference: Vec<u8>,
}

impl MediaLibraryEntry {
    /// Reconstructs an entry from the exact Telegram document identity saved
    /// with a durable outbound operation.
    #[must_use]
    pub fn from_remote_parts(
        id: i64,
        label: String,
        kind: MediaLibraryKind,
        access_hash: i64,
        file_reference: Vec<u8>,
    ) -> Self {
        Self {
            id,
            label,
            kind,
            access_hash,
            file_reference,
        }
    }

    /// Returns the Telegram media-library family.
    #[must_use]
    pub const fn kind(&self) -> MediaLibraryKind {
        self.kind
    }

    /// Returns the remote document access hash.
    #[must_use]
    pub const fn access_hash(&self) -> i64 {
        self.access_hash
    }

    /// Returns the exact remote file reference needed to replay the send.
    #[must_use]
    pub fn file_reference(&self) -> &[u8] {
        &self.file_reference
    }
}

/// One Telegram contact card submission.
pub struct ContactCardSend {
    /// Destination Chat.
    pub chat: ChatId,

    /// Telegram-compatible telephone number.
    pub phone_number: String,

    /// Contact first name.
    pub first_name: String,

    /// Optional contact last name.
    pub last_name: String,

    /// Direct reply target.
    pub reply_to: Option<MessageId>,

    /// Active Thread root.
    pub thread_root: Option<MessageId>,

    /// User topic inside an administrator-owned monoforum.
    pub monoforum_peer: Option<ChatId>,

    /// Stable Message idempotency identifier.
    pub random_id: i64,
}

/// One Telegram-owned media-library submission.
pub struct LibraryMediaSend {
    /// Destination Chat.
    pub chat: ChatId,

    /// Previously browsed Telegram media entry.
    pub entry: MediaLibraryEntry,

    /// Direct reply target.
    pub reply_to: Option<MessageId>,

    /// Active Thread root.
    pub thread_root: Option<MessageId>,

    /// User topic inside an administrator-owned monoforum.
    pub monoforum_peer: Option<ChatId>,

    /// Stable Message idempotency identifier.
    pub random_id: i64,
}

pub(in crate::client) struct InputMediaSend {
    pub(in crate::client) peer: tl::enums::InputPeer,
    pub(in crate::client) media: tl::enums::InputMedia,
    pub(in crate::client) message: String,
    pub(in crate::client) reply_to: Option<MessageId>,
    pub(in crate::client) thread_root: Option<MessageId>,
    pub(in crate::client) monoforum_peer: Option<ChatId>,
    pub(in crate::client) random_id: i64,
}
