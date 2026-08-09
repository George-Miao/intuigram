use super::{StoredChat, StoredFolder, StoredMessage, StoredSavedDialog, StoredTopic, SyncCursor};

/// Durable Draft value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredDraft {
    /// Owning Chat.
    pub chat_id: i64,

    /// Thread root, or `None` for the root Chat Draft.
    pub thread_root: Option<i64>,

    /// Per-user dialog inside an administrator-owned monoforum.
    pub saved_peer: Option<i64>,

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

    /// Original peer for a filtered Saved Messages or monoforum history.
    pub saved_peer: Option<i64>,

    /// Message anchoring the viewport.
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

    /// Cached ordered Saved Messages and monoforum dialogs.
    pub saved_dialogs: Vec<StoredSavedDialog>,

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
