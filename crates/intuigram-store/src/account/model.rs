/// Durable connection material for one Telegram data-center authorization.
#[derive(Clone, Eq, PartialEq)]
pub struct SessionMaterial {
    /// Telegram data-center number.
    pub dc_id: i32,
    /// Direct TCP endpoint associated with this authorization.
    pub endpoint: String,
    /// Secret authorization key. Never include this value in diagnostics.
    auth_key: [u8; 256],
    /// Difference between local and Telegram server time.
    pub time_offset: i32,
    /// Most recently known server salt.
    pub first_salt: i64,
}

impl fmt::Debug for SessionMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SessionMaterial")
            .field("dc_id", &self.dc_id)
            .field("endpoint", &self.endpoint)
            .field("auth_key", &"[REDACTED]")
            .field("time_offset", &self.time_offset)
            .field("first_salt", &self.first_salt)
            .finish()
    }
}

impl SessionMaterial {
    /// Creates durable session material.
    #[must_use]
    pub const fn new(
        dc_id: i32,
        endpoint: String,
        auth_key: [u8; 256],
        time_offset: i32,
        first_salt: i64,
    ) -> Self {
        Self {
            dc_id,
            endpoint,
            auth_key,
            time_offset,
            first_salt,
        }
    }

    /// Copies the secret key into the protocol adapter.
    #[must_use]
    pub const fn auth_key(&self) -> [u8; 256] {
        self.auth_key
    }
}

/// Telegram synchronization cursor committed with normalized records.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SyncCursor {
    /// Logical synchronization scope, normally `account` or a Channel ID.
    pub scope: String,

    /// Telegram persistent timestamp.
    pub pts: i32,

    /// Telegram secret-chat timestamp retained for protocol completeness.
    pub qts: i32,

    /// Telegram server date.
    pub date: i32,

    /// Telegram global update sequence.
    pub seq: i32,
}

/// Store-owned normalized Folder record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredFolder {
    /// Telegram Folder ID.
    pub id: i32,

    /// Display title.
    pub title: String,

    /// Aggregate unread count.
    pub unread: u32,
}

/// Store-owned normalized Chat record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredChat {
    /// Stable marked Telegram peer ID.
    pub id: i64,

    /// Stable textual normalized Chat kind.
    pub kind: String,

    /// Display title.
    pub title: String,

    /// Last-message fallback.
    pub preview: String,

    /// Idle Chat metadata normalized for display.
    pub status: String,

    /// Unread count.
    pub unread: u32,

    /// Telegram pin state.
    pub pinned: bool,

    /// Whether current server rights permit pinning Messages.
    pub can_pin_messages: bool,

    /// Whether opening this Chat descends through a Topic list.
    pub has_topics: bool,

    /// Folder IDs in which the Chat appears.
    pub folders: Vec<i32>,
}

/// Store-owned normalized forum Topic record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredTopic {
    /// Owning Chat.
    pub chat_id: i64,

    /// Stable Topic identity and root Message.
    pub id: i64,

    /// Display title.
    pub title: String,

    /// Latest Message fallback.
    pub preview: String,

    /// Latest Message timestamp.
    pub timestamp: String,

    /// Topic-local unread count.
    pub unread: u32,

    /// Telegram pin state.
    pub pinned: bool,

    /// Whether posting is closed.
    pub closed: bool,

    /// Whether General is hidden by Telegram.
    pub hidden: bool,

    /// Telegram RGB icon color.
    pub icon_color: u32,

    /// Custom emoji icon, when any.
    pub icon_emoji_id: Option<i64>,

    /// Latest Message identity, when any.
    pub top_message_id: Option<i64>,

    /// Server Draft text, when any.
    pub draft_text: Option<String>,

    /// Server Draft reply target, when any.
    pub draft_reply_to: Option<i64>,
}

/// Store-owned normalized Message record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredMessage {
    /// Owning Chat.
    pub chat_id: i64,

    /// Telegram or pending local Message ID.
    pub id: i64,

    /// Sender display fallback.
    pub sender: String,

    /// Searchable semantic text fallback.
    pub body: String,

    /// Compact presentation timestamp.
    pub timestamp: String,

    /// Stable textual direction.
    pub direction: String,

    /// Stable textual delivery state.
    pub delivery: String,

    /// Replied-to Message, when any.
    pub reply_to: Option<i64>,

    /// Thread root, or `None` for root Chat history.
    pub thread_root: Option<i64>,

    /// Stable semantic content kind.
    pub content_kind: String,

    /// Forward-compatible normalized metadata.
    pub metadata: String,
}

/// Normalized durable mutation applied with an update cursor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StoredMutation {
    /// Change whether the Account may pin Messages in one Chat.
    SetChatPinPermission {
        /// Owning Chat.
        chat_id: i64,

        /// Current effective permission.
        can_pin_messages: bool,
    },

    /// Change whether opening one Chat descends through a Topic list.
    SetChatHasTopics {
        /// Owning Chat.
        chat_id: i64,

        /// Current Telegram feature state.
        has_topics: bool,
    },

    /// Change pinned state for Message IDs in one Chat.
    SetMessagesPinned {
        /// Owning Chat.
        chat_id: i64,

        /// Telegram Message IDs whose state changed.
        ids: Vec<i64>,

        /// New pinned state.
        pinned: bool,
    },

    /// Delete Message IDs, optionally scoped to one Channel Chat.
    DeleteMessages {
        /// Channel Chat for channel-local IDs; `None` for account-wide IDs.
        chat_id: Option<i64>,

        /// Telegram Message IDs to remove.
        ids: Vec<i64>,
    },

    /// Advance incoming unread state or outgoing read receipts.
    ReadHistory {
        /// Owning Chat.
        chat_id: i64,

        /// Highest affected Message ID.
        max_id: i64,

        /// Whether outgoing Messages became read by recipients.
        outgoing: bool,

        /// Remaining incoming unread count, when supplied.
        unread: Option<u32>,
    },

    /// Move one Chat into or out of Archive.
    MoveArchive {
        /// Chat whose root Folder changes.
        chat_id: i64,

        /// Whether the Chat is archived.
        archived: bool,
    },
}

/// One atomic synchronized-cache commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncBatch {
    /// Independent cursors advanced by this exact record set.
    pub cursors: Vec<SyncCursor>,

    /// Folder records to upsert in server order.
    pub folders: Vec<StoredFolder>,

    /// Chat records to upsert.
    pub chats: Vec<StoredChat>,

    /// Complete Telegram Chat order, when this batch is an authoritative
    /// bootstrap.
    pub chat_order: Option<Vec<i64>>,

    /// Message records to upsert.
    pub messages: Vec<StoredMessage>,

    /// Deletes, read-state changes, and Folder moves in this update.
    pub mutations: Vec<StoredMutation>,
}

/// Durable Draft value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredDraft {
    /// Owning Chat.
    pub chat_id: i64,

    /// Thread root, or `None` for the root Chat Draft.
    pub thread_root: Option<i64>,

    /// Draft text.
    pub text: String,

    /// Replied-to Message, when any.
    pub reply_to: Option<i64>,

    /// Unix timestamp used for last-writer conflict resolution.
    pub modified_at: i64,
}

/// Last durable navigation target for one Account.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredSelection {
    /// Telegram Folder ID selected in the Chat list.
    pub folder_id: i32,

    /// Selected Chat, or `None` when no Chat is selected.
    pub chat_id: Option<i64>,

    /// Message anchoring the restored Transcript viewport.
    pub anchor_message_id: Option<i64>,

    /// Per-Chat and per-Thread Transcript positions retained for this Account.
    pub transcript_anchors: Vec<StoredTranscriptAnchor>,
}

/// One durable Transcript position within an Account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoredTranscriptAnchor {
    /// Owning Chat.
    pub chat_id: i64,

    /// Thread root, or `None` for root Chat history.
    pub thread_root: Option<i64>,

    /// Message anchoring the restored viewport.
    pub message_id: i64,
}

/// Immediately renderable durable Account cache.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CachedAccount {
    /// Last durable synchronization cursors.
    pub cursors: Vec<SyncCursor>,

    /// Folders in display order.
    pub folders: Vec<StoredFolder>,

    /// Cached Chats.
    pub chats: Vec<StoredChat>,

    /// Cached ordered Topic projections.
    pub topics: Vec<StoredTopic>,

    /// Cached Messages.
    pub messages: Vec<StoredMessage>,

    /// Pinned Message projection, independently of contiguous recent history.
    pub pinned_messages: Vec<StoredMessage>,

    /// Current durable Drafts.
    pub drafts: Vec<StoredDraft>,

    /// Last selected Folder and Chat, when the application saved one.
    pub selection: Option<StoredSelection>,

    /// Chats whose original media is protected from ordinary cache eviction.
    pub offline_chats: Vec<i64>,
}
use super::*;
