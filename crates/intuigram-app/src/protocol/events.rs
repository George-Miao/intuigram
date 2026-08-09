use super::*;

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

    /// Telegram changed one Chat's effective notification mute state.
    ChatMuteChanged { chat: ChatId, muted: bool },

    /// One Chat's Account-local offline-media policy was durably changed.
    ChatMediaOfflineChanged(OfflineMediaPolicy),

    /// One Chat's offline-media policy could not be changed.
    ChatMediaOfflineFailed(OfflineMediaFailure),

    /// One forum or topic-enabled bot Chat's Topic projection loaded.
    TopicsLoaded(TopicListView),

    /// A Topic projection could not be refreshed.
    TopicsLoadFailed(TopicLoadFailure),

    /// One Saved Messages 2.0 dialog projection loaded.
    SavedDialogsLoaded(SavedDialogListView),

    /// A Saved Messages 2.0 dialog projection could not be refreshed.
    SavedDialogsLoadFailed(SavedDialogLoadFailure),

    /// One Message's original media is protected from ordinary eviction.
    MediaCachedOffline(OfflineMediaTarget),

    /// One Message could not be retained for offline use.
    MediaCacheOfflineFailed(OfflineMediaFailure),

    /// A requested Chat Folder membership change was acknowledged.
    FolderMembershipChanged {
        chat: ChatId,

        folder: i32,

        included: bool,
    },

    /// A Folder lifecycle request was accepted and normalized.
    FolderOperationCompleted {
        result: FolderOperationResult,

        reconciliation: Option<Box<FolderReconciliation>>,
    },

    /// A requested authoritative Folder projection became available.
    FolderReconciled(Box<FolderReconciliation>),

    /// A Folder refresh failed without invalidating the accepted mutation.
    FolderReconciliationFailed(String),

    /// A Folder lifecycle request failed without invalidating connectivity.
    FolderOperationFailed(String),

    /// A Telegram media library query completed.
    RichMediaLibraryReady {
        kind: RichMediaLibraryKind,
        items: Vec<RichMediaItemView>,
    },

    /// A Telegram media library query failed without changing the Draft.
    RichMediaLibraryFailed(String),

    /// A correlated Telegram place search completed.
    PlaceSearchReady {
        chat: ChatId,

        query: String,

        near: Option<GeoPointView>,

        places: Vec<PlaceView>,
    },

    /// A correlated Telegram place search failed.
    PlaceSearchFailed {
        chat: ChatId,

        query: String,

        near: Option<GeoPointView>,

        reason: String,
    },

    /// One rich-media send was accepted by Telegram.
    RichMediaAcknowledged {
        chat: ChatId,

        local_id: MessageId,

        server_id: MessageId,
    },

    /// One rich-media send failed and remains visible in the Transcript.
    RichMediaFailed {
        chat: ChatId,
        local_id: MessageId,
        reason: String,
    },

    /// Server-owned Scheduled Message history loaded for one Chat.
    ScheduledMessagesReady {
        chat: ChatId,

        saved_peer: Option<ChatId>,
        messages: Vec<ScheduledMessageView>,
    },

    /// A Scheduled Message mutation completed and returned a fresh projection.
    ScheduledOperationCompleted {
        chat: ChatId,

        saved_peer: Option<ChatId>,
        messages: Vec<ScheduledMessageView>,
        notice: String,
    },

    /// Scheduled Message work failed without changing ordinary Message History.
    ScheduledOperationFailed {
        chat: ChatId,

        saved_peer: Option<ChatId>,
        reason: String,
    },

    /// A nonfatal Telegram operation failed.
    OperationFailed(String),

    /// The backend is quiescent and the composition root may change Accounts.
    AccountLifecycleReady(AccountLifecycle),

    /// A platform or Telegram action completed with a visible result.
    OperationCompleted(String),

    /// A live update introduced a Chat absent from the synchronized cache.
    ChatDiscovered { chat: ChatView },

    /// A new or acknowledged Message belongs in a Chat history.
    MessageAdded {
        chat: ChatId,
        message: Box<MessageView>,
    },

    /// An existing Message changed content or metadata.
    MessageUpdated {
        chat: ChatId,

        message: Box<MessageView>,
    },

    /// Telegram revealed newer ordered paid-media child state for one Message.
    PaidMediaItemsUpdated {
        chat: ChatId,

        message: MessageId,

        items: Vec<PaidMediaItemView>,
    },

    /// Telegram changed pinned state for Messages without replacing their
    /// content.
    MessagesPinChanged {
        chat: ChatId,

        ids: Vec<MessageId>,

        pinned: bool,
    },

    /// A terminal edit failure restored the attempted text for correction.
    MessageEditFailed {
        chat: ChatId,

        message: MessageId,

        text: String,

        attachments: Vec<AttachmentView>,

        reason: String,
    },

    /// Telegram removed Messages from one Chat or the account-wide ID space.
    MessagesDeleted {
        chat: Option<ChatId>,

        ids: Vec<MessageId>,
    },

    /// Telegram advanced incoming unread state or outgoing read receipts.
    HistoryRead {
        chat: ChatId,

        saved_peer: Option<ChatId>,

        max_id: MessageId,

        outgoing: bool,

        unread: Option<u32>,
    },

    /// Telegram moved a Chat into or out of Archive.
    ChatArchiveChanged { chat: ChatId, archived: bool },

    /// Telegram changed whether the Account may pin Messages in a Chat.
    ChatPinPermissionChanged {
        chat: ChatId,

        can_pin_messages: bool,
    },
    /// Telegram changed whether one Chat presents Topic navigation.
    ChatTopicsChanged(TopicAvailability),
    /// A requested Chat history became available.
    ChatLoaded {
        chat: ChatId,

        status: Option<String>,

        messages: Vec<MessageView>,

        pinned_messages: Vec<MessageView>,
    },

    /// A requested root or Thread history could not be refreshed.
    HistoryLoadFailed {
        chat: ChatId,

        thread_root: Option<MessageId>,

        saved_peer: Option<ChatId>,

        reason: String,
    },

    /// A requested Thread history became available.
    ThreadLoaded {
        chat: ChatId,

        root: MessageId,

        saved_peer: Option<ChatId>,

        messages: Vec<MessageView>,
    },

    /// Saved Messages filtered to one original peer became available.
    SavedHistoryLoaded {
        chat: ChatId,

        peer: ChatId,

        messages: Vec<MessageView>,
    },

    /// One original-peer Saved Messages history could not be refreshed.
    SavedHistoryLoadFailed {
        chat: ChatId,

        peer: ChatId,

        reason: String,
    },
    /// Native clipboard content became available for a Composer.
    ClipboardReady {
        chat: ChatId,

        thread_root: Option<MessageId>,

        saved_peer: Option<ChatId>,

        text: Option<String>,

        attachments: Vec<AttachmentView>,
    },
    /// No external path picker is configured, so the built-in field is needed.
    AttachmentPathRequired {
        chat: ChatId,

        thread_root: Option<MessageId>,

        saved_peer: Option<ChatId>,
    },
    /// Telegram acknowledged an optimistic local Message.
    MessageAcknowledged { chat: ChatId, local_id: MessageId },

    /// Telegram acknowledged a durable edit without replacing unrelated
    /// cached Message metadata.
    MessageEditAcknowledged {
        chat: ChatId,

        message: MessageId,

        text: String,

        entities: Vec<TextEntity>,
    },

    /// Telegram returned authoritative media state for one durable mutation.
    MessageMediaUpdated {
        chat: ChatId,

        message: MessageId,

        media: MediaCard,
    },

    /// A pending send reached a terminal failure.
    MessageFailed {
        chat: ChatId,

        local_id: MessageId,

        thread_root: Option<MessageId>,

        saved_peer: Option<ChatId>,

        text: String,

        reason: String,
    },

    /// One durable outbound operation was admitted or changed lifecycle.
    OutboxChanged(OutboxItemView),

    /// One terminal or acknowledged operation left the durable Outbox.
    OutboxRemoved { item: OutboxKey },

    /// Telegram acknowledged a Scheduled Message mutation; its server-owned
    /// projection must now be reloaded.
    ScheduledOperationAcknowledged {
        chat: ChatId,

        saved_peer: Option<ChatId>,

        notice: String,
    },

    /// A poll send failed and its structured editor contents remain
    /// recoverable.
    PollFailed {
        chat: ChatId,

        local_id: MessageId,

        thread_root: Option<MessageId>,

        saved_peer: Option<ChatId>,

        text: String,

        reason: String,
    },

    /// Telegram resolved a supported internal username link.
    TelegramLinkResolved { chat: ChatView },

    /// Media bytes were saved to the configured download directory.
    DownloadReady {
        chat: ChatId,

        download: DownloadView,
    },

    /// An image preview became available without creating a user download.
    MediaPreviewReady(MediaPreviewView),

    /// A peer avatar became available for rendering.
    AvatarReady(AvatarView),

    /// Telegram changed or removed one peer's avatar revision.
    AvatarChanged { peer: ChatId, id: Option<AvatarId> },

    /// A known peer avatar could not be loaded or decoded.
    AvatarFailed { avatar: AvatarRef },

    /// An automatic image preview could not be loaded.
    MediaPreviewFailed { chat: ChatId, message: MessageId },
}
