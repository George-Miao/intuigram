use super::*;

/// Initial synchronized data supplied by an adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bootstrap {
    /// Connectivity represented by this initial data source.
    pub connection: ConnectionState,

    /// Active Account display name.
    pub account_name: String,

    /// Stable Account-scoped identity for notification replacement.
    pub notification_identity: String,

    /// Chats whose effective Telegram notification setting is muted.
    pub muted_chats: Vec<ChatId>,

    /// Chats whose original media is retained outside ordinary cache eviction.
    pub offline_chats: Vec<ChatId>,

    /// Registered Accounts available without restarting Intuigram.
    pub accounts: Vec<AccountView>,

    /// Last durable Folder and Chat selection, when one has been saved.
    pub restored_selection: Option<SelectionView>,

    /// Durable Transcript positions for every previously visited history.
    pub transcript_anchors: Vec<TranscriptAnchorView>,

    /// Synchronized Telegram Folders.
    pub folders: Vec<FolderView>,

    /// Editable metadata for synchronized custom Folders.
    pub folder_details: Vec<FolderDetailsView>,

    /// Chats in the active Folder.
    pub chats: Vec<ChatView>,

    /// Cached Topic lists available before Telegram reconnects.
    pub topic_lists: Vec<TopicListView>,

    /// Versioned peer avatars known by the adapter.
    pub avatar_peers: Vec<AvatarRef>,

    /// Messages for the initially active Chat.
    pub messages: Vec<MessageView>,

    /// Cached pinned-Message projections by Chat.
    pub pinned_messages: Vec<HistoryView>,

    /// Durable root and Thread Drafts for cached Chats.
    pub drafts: Vec<DraftView>,

    /// Cached histories for immediate Chat switching.
    pub histories: Vec<HistoryView>,
}
