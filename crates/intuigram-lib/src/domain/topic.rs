use super::{ChatId, MessageId};

/// Stable identifier for one Telegram forum Topic.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TopicId(pub i64);

impl TopicId {
    /// Returns the root Message used by Telegram to address this Topic.
    #[must_use]
    pub const fn root_message(self) -> MessageId {
        MessageId(self.0)
    }
}

/// Server Draft attached to one Topic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TopicDraftView {
    /// Unsent text.
    pub text: String,

    /// Direct reply target, when any.
    pub reply_to: Option<MessageId>,
}

/// Dense Topic-list row independent of the parent Chat row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TopicView {
    /// Stable Topic identity and root Message.
    pub id: TopicId,

    /// Display title.
    pub title: String,

    /// Latest Message fallback.
    pub preview: String,

    /// Latest Message timestamp.
    pub timestamp: String,

    /// Number of unread Messages inside this Topic.
    pub unread: u32,

    /// Whether Telegram pins this Topic.
    pub pinned: bool,

    /// Whether posting is closed.
    pub closed: bool,

    /// Whether General is hidden by Telegram. Intuigram still presents it.
    pub hidden: bool,

    /// Telegram's stable RGB Topic icon color.
    pub icon_color: u32,

    /// Custom emoji used as the Topic icon, when present.
    pub icon_emoji_id: Option<i64>,

    /// Latest Message in this Topic, when Telegram supplied one.
    pub top_message: Option<MessageId>,

    /// Server Draft restored when no newer local Draft exists.
    pub draft: Option<TopicDraftView>,
}

/// Cached Topic projection for one Chat.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TopicListView {
    /// Owning forum Supergroup or topic-enabled bot Chat.
    pub chat: ChatId,

    /// Telegram order, including General even when hidden.
    pub topics: Vec<TopicView>,
}

/// Telegram feature state controlling Topic navigation for one Chat.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TopicAvailability {
    /// Chat whose navigation shape changed.
    pub chat: ChatId,

    /// Whether opening descends through a Topic list.
    pub has_topics: bool,
}

/// Failed Topic projection refresh safe to cross adapter seams.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TopicLoadFailure {
    /// Owning Chat.
    pub chat: ChatId,

    /// User-facing semantic failure.
    pub reason: String,
}
