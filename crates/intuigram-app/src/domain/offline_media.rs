use super::{ChatId, MessageId};

/// Account-local policy controlling protected original-media retention.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OfflineMediaPolicy {
    /// Chat whose policy changes.
    pub chat: ChatId,

    /// Whether original media must remain available outside ordinary eviction.
    pub keep: bool,
}

/// One Message whose original media should remain available offline.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct OfflineMediaTarget {
    /// Chat containing the media Message.
    pub chat: ChatId,

    /// Message containing the original media.
    pub message: MessageId,
}

/// A failed offline-media policy or original-media operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfflineMediaFailure {
    /// Chat containing the failed operation.
    pub chat: ChatId,

    /// Specific Message for a media-retention failure, when applicable.
    pub message: Option<MessageId>,

    /// User-facing semantic failure.
    pub reason: String,
}
