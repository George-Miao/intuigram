/// Initial synchronized data supplied by an adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bootstrap {
    /// Connectivity represented by this initial data source.
    pub connection: ConnectionState,

    /// Active Account display name.
    pub account_name: String,

    /// Stable Account-scoped identity for notification replacement.
    pub notification_identity: String,

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
    /// Messages for the initially active Chat.
    pub messages: Vec<MessageView>,

    /// Cached pinned-Message projections by Chat.
    pub pinned_messages: Vec<HistoryView>,

    /// Durable root and Thread Drafts for cached Chats.
    pub drafts: Vec<DraftView>,

    /// Cached histories for immediate Chat switching.
    pub histories: Vec<HistoryView>,
}

/// Results reported by external adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdapterEvent {
    /// Initial synchronized data became available.
    Bootstrap(Bootstrap),

    /// A disconnected Account reconnected with a fresh synchronized snapshot.
    ConnectionRestored(Bootstrap),

    /// Telegram connectivity changed.
    ConnectionChanged(ConnectionState),

    /// Automatic connection attempts entered cooldown after a failure.
    ConnectionFailed(String),

    /// A requested Chat Folder membership change was acknowledged.
    FolderMembershipChanged {
        /// Chat whose membership changed.
        chat: ChatId,

        /// Folder added to or removed from the Chat.
        folder: i32,

        /// Whether the Chat now belongs to the Folder.
        included: bool,
    },

    /// A Folder lifecycle request was accepted and normalized.
    FolderOperationCompleted {
        /// Accepted operation.
        result: FolderOperationResult,

        /// Best-effort fresh server projection after a mutating operation.
        reconciliation: Option<Box<FolderReconciliation>>,
    },

    /// A Folder lifecycle request failed without invalidating connectivity.
    FolderOperationFailed(String),

    /// A Telegram media library query completed.
    RichMediaLibraryReady {
        kind: RichMediaLibraryKind,
        items: Vec<RichMediaItemView>,
    },

    /// A Telegram media library query failed without changing the Draft.
    RichMediaLibraryFailed(String),

    /// One rich-media send was accepted by Telegram.
    RichMediaAcknowledged { chat: ChatId, local_id: MessageId },

    /// One rich-media send failed and remains visible in the Transcript.
    RichMediaFailed {
        chat: ChatId,
        local_id: MessageId,
        reason: String,
    },

    /// Server-owned Scheduled Message history loaded for one Chat.
    ScheduledMessagesReady {
        /// Owning Chat.
        chat: ChatId,
        /// Complete current server projection.
        messages: Vec<ScheduledMessageView>,
    },

    /// A Scheduled Message mutation completed and returned a fresh projection.
    ScheduledOperationCompleted {
        /// Owning Chat.
        chat: ChatId,
        /// Complete current server projection.
        messages: Vec<ScheduledMessageView>,
        /// User-facing completion summary.
        notice: String,
    },

    /// Scheduled Message work failed without changing ordinary Message History.
    ScheduledOperationFailed {
        /// Owning Chat.
        chat: ChatId,
        /// Failure safe for display.
        reason: String,
    },

    /// A nonfatal Telegram operation failed.
    OperationFailed(String),

    /// The backend is quiescent and the composition root may change Accounts.
    AccountLifecycleReady(AccountLifecycle),

    /// A platform or Telegram action completed with a visible result.
    OperationCompleted(String),

    /// A live update introduced a Chat absent from the synchronized cache.
    ChatDiscovered {
        /// Safe fallback Chat metadata available before dialog reconciliation.
        chat: ChatView,
    },

    /// A new or acknowledged Message belongs in a Chat history.
    MessageAdded {
        /// Chat that owns the Message.
        chat: ChatId,
        /// Newly available Message.
        message: Box<MessageView>,
    },

    /// An existing Message changed content or metadata.
    MessageUpdated {
        /// Chat that owns the Message.
        chat: ChatId,

        /// Complete replacement Message.
        message: Box<MessageView>,
    },

    /// Telegram changed pinned state for Messages without replacing their
    /// content.
    MessagesPinChanged {
        /// Chat containing the affected Messages.
        chat: ChatId,

        /// Telegram Message IDs whose pinned state changed.
        ids: Vec<MessageId>,

        /// New pinned state.
        pinned: bool,
    },

    /// A terminal edit failure restored the attempted text for correction.
    MessageEditFailed {
        /// Chat containing the Message.
        chat: ChatId,

        /// Message whose edit failed.
        message: MessageId,

        /// Attempted replacement text.
        text: String,

        /// User-facing semantic failure.
        reason: String,
    },

    /// Telegram removed Messages from one Chat or the account-wide ID space.
    MessagesDeleted {
        /// Channel Chat for channel-local IDs; `None` for account-wide IDs.
        chat: Option<ChatId>,

        /// Removed Telegram Message IDs.
        ids: Vec<MessageId>,
    },

    /// Telegram advanced incoming unread state or outgoing read receipts.
    HistoryRead {
        /// Chat whose read state changed.
        chat: ChatId,

        /// Highest affected Message ID.
        max_id: MessageId,

        /// `true` when recipients read this Account's outgoing Messages.
        outgoing: bool,

        /// Remaining incoming unread count when supplied by Telegram.
        unread: Option<u32>,
    },

    /// Telegram moved a Chat into or out of Archive.
    ChatArchiveChanged {
        /// Chat whose root Folder changed.
        chat: ChatId,

        /// Whether the Chat is now archived.
        archived: bool,
    },

    /// Telegram changed whether the Account may pin Messages in a Chat.
    ChatPinPermissionChanged {
        /// Chat whose effective rights changed.
        chat: ChatId,

        /// Whether Message pinning is currently permitted.
        can_pin_messages: bool,
    },
    /// A requested Chat history became available.
    ChatLoaded {
        /// Chat whose history was loaded.
        chat: ChatId,

        /// Chronological loaded history.
        messages: Vec<MessageView>,

        /// Bounded pinned-Message projection, independent of recent history.
        pinned_messages: Vec<MessageView>,
    },

    /// A requested root or Thread history could not be refreshed.
    HistoryLoadFailed {
        /// Parent Chat.
        chat: ChatId,

        /// Thread root, or `None` for root Chat history.
        thread_root: Option<MessageId>,

        /// User-facing semantic failure.
        reason: String,
    },

    /// A requested Thread history became available.
    ThreadLoaded {
        /// Parent Chat.
        chat: ChatId,

        /// Root Message of the Thread.
        root: MessageId,

        /// Chronological Thread history.
        messages: Vec<MessageView>,
    },
    /// Native clipboard content became available for a Composer.
    ClipboardReady {
        /// Chat whose Composer requested the paste.
        chat: ChatId,

        /// Thread Composer, when applicable.
        thread_root: Option<MessageId>,

        /// Text to insert.
        text: Option<String>,

        /// Adapter-owned attachment candidates.
        attachments: Vec<AttachmentView>,
    },
    /// Telegram acknowledged an optimistic local Message.
    MessageAcknowledged {
        /// Owning Chat.
        chat: ChatId,

        /// Pending local Message ID.
        local_id: MessageId,
    },

    /// A pending send reached a terminal failure.
    MessageFailed {
        /// Owning Chat.
        chat: ChatId,

        /// Pending local Message ID.
        local_id: MessageId,

        /// Thread Composer, when applicable.
        thread_root: Option<MessageId>,

        /// Draft text that must remain recoverable.
        text: String,

        /// User-facing semantic failure.
        reason: String,
    },

    /// A poll send failed and its structured editor contents remain
    /// recoverable.
    PollFailed {
        /// Owning Chat.
        chat: ChatId,

        /// Pending local Message ID.
        local_id: MessageId,

        /// Thread Composer, when applicable.
        thread_root: Option<MessageId>,

        /// Complete poll editor text: question, then one option per line.
        text: String,

        /// User-facing semantic failure.
        reason: String,
    },

    /// Telegram resolved a supported internal username link.
    TelegramLinkResolved {
        /// Chat reached by the link.
        chat: ChatView,
    },

    /// Media bytes were saved to the configured download directory.
    DownloadReady {
        /// Chat containing the downloaded Message.
        chat: ChatId,

        /// Adapter-owned completed download.
        download: DownloadView,
    },

    /// An image preview became available without creating a user download.
    MediaPreviewReady(MediaPreviewView),

    /// An automatic image preview could not be loaded.
    MediaPreviewFailed {
        /// Chat containing the Message.
        chat: ChatId,

        /// Message whose preview failed.
        message: MessageId,
    },
}
use crate::domain::*;

mod effects;
mod input;
mod intents;
mod view;

pub use effects::*;
pub use input::*;
pub use intents::*;
pub use view::*;
