/// Side effects requested from adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    /// Ask the composition root to change the active Account lifecycle.
    AccountLifecycle { request: AccountLifecycle },
    /// Create, edit, reorder, share, or delete a Telegram Folder.
    FolderOperation { operation: FolderOperation },
    /// Refresh the authoritative Folder and Chat-membership projection.
    RefreshFolders,
    /// Load recent or saved Telegram media for Composer selection.
    BrowseRichMedia { kind: RichMediaLibraryKind },
    /// Send a previously browsed Telegram media item.
    SendLibraryMedia {
        chat: ChatId,

        item: RichMediaItemId,

        local_id: MessageId,

        reply_to: Option<MessageId>,

        thread_root: Option<MessageId>,

        saved_peer: Option<ChatId>,
    },
    /// Read and upload one exact local path with explicit media semantics.
    SendRichMediaFile {
        chat: ChatId,

        path: String,

        kind: RichMediaUploadKind,

        local_id: MessageId,

        reply_to: Option<MessageId>,

        thread_root: Option<MessageId>,

        saved_peer: Option<ChatId>,
    },
    /// Capture and send a voice or circular video note.
    RecordRichMedia {
        chat: ChatId,

        kind: RichMediaUploadKind,

        seconds: u32,

        device: String,

        local_id: MessageId,

        reply_to: Option<MessageId>,

        thread_root: Option<MessageId>,

        saved_peer: Option<ChatId>,
    },
    /// Send a Telegram contact card from Composer fields.
    SendContact {
        chat: ChatId,

        phone: String,

        first_name: String,

        last_name: String,

        local_id: MessageId,

        reply_to: Option<MessageId>,

        thread_root: Option<MessageId>,

        saved_peer: Option<ChatId>,
    },
    /// Load server-owned Scheduled Message history independently of Transcript
    /// history.
    LoadScheduledMessages {
        chat: ChatId,

        saved_peer: Option<ChatId>,
    },
    /// Apply one typed Scheduled Message mutation.
    ScheduledOperation {
        chat: ChatId,

        saved_peer: Option<ChatId>,

        request: ScheduledRequest,
    },
    /// Alert the user about an incoming Message outside the visibly read Chat.
    Notify { identity: String, chat: ChatId },
    /// Start a connection attempt immediately.
    Reconnect,
    /// Add or remove the Active Chat from one Telegram Folder.
    SetChatFolder {
        chat: ChatId,

        folder: i32,

        included: bool,
    },
    /// Persist whether one Chat's media is protected from ordinary eviction.
    SetChatMediaOffline(OfflineMediaPolicy),
    /// Retain one Message's original media outside ordinary eviction.
    CacheMediaOffline(OfflineMediaTarget),
    /// Load recent history for the selected Chat.
    LoadChat {
        chat: ChatId,

        selection: Option<SelectionView>,

        transcript_anchors: Vec<TranscriptAnchorView>,
    },
    /// Load the complete ordered Topic projection for one Chat.
    LoadTopics(ChatId),
    /// Load the complete per-peer projection for Saved Messages or a monoforum.
    LoadSavedDialogs(ChatId),
    /// Load Saved Messages or a monoforum filtered to one original peer.
    LoadSavedHistory { chat: ChatId, peer: ChatId },
    /// Persist navigation when no Chat load is needed.
    SaveSelection {
        folder: i32,

        chat: Option<ChatId>,

        message: Option<MessageId>,

        transcript_anchors: Vec<TranscriptAnchorView>,
    },
    /// Load an ordinary Message Thread or Channel comments.
    LoadThread {
        chat: ChatId,

        root: MessageId,

        saved_peer: Option<ChatId>,
    },
    /// Advance read state for an ordinary Thread or Channel comments.
    ReadThread {
        chat: ChatId,

        root: MessageId,

        max_id: MessageId,

        saved_peer: Option<ChatId>,
    },
    /// Advance read state for visible root Chat history.
    ReadHistory {
        chat: ChatId,

        max_id: MessageId,

        saved_peer: Option<ChatId>,
    },
    /// Query the native clipboard without blocking terminal input.
    ReadClipboard {
        chat: ChatId,

        thread_root: Option<MessageId>,

        saved_peer: Option<ChatId>,
    },
    /// Ask the configured platform picker for a local attachment path.
    PickAttachment {
        chat: ChatId,

        thread_root: Option<MessageId>,

        saved_peer: Option<ChatId>,
    },
    /// Validate and retain one exact local attachment path.
    SelectAttachment {
        chat: ChatId,

        thread_root: Option<MessageId>,

        saved_peer: Option<ChatId>,

        path: String,
    },
    /// Persist a changed Draft before any saved indication is emitted.
    SaveDraft {
        chat: ChatId,

        thread_root: Option<MessageId>,

        saved_peer: Option<ChatId>,

        text: String,

        reply_to: Option<MessageId>,
    },
    /// Send one text Message, optionally as a reply.
    SendMessage {
        chat: ChatId,
        text: String,

        entities: Vec<TextEntity>,

        link_preview: bool,

        reply_to: Option<MessageId>,

        thread_root: Option<MessageId>,

        saved_peer: Option<ChatId>,

        attachments: Vec<AttachmentId>,

        local_id: MessageId,
    },
    /// Send a Telegram poll with at least two choices.
    SendPoll {
        chat: ChatId,

        question: String,

        options: Vec<String>,

        reply_to: Option<MessageId>,

        thread_root: Option<MessageId>,

        saved_peer: Option<ChatId>,

        local_id: MessageId,
    },
    /// Replace the text of one outgoing Message.
    EditMessage {
        chat: ChatId,

        message: Box<MessageView>,

        draft_text: String,

        attachments: Vec<AttachmentId>,

        draft_attachments: Vec<AttachmentView>,
    },
    /// Delete one or more Messages from Telegram and durable storage.
    DeleteMessages {
        chat: ChatId,

        messages: Vec<MessageId>,
    },
    /// Forward one or more Messages between cloud Chats.
    ForwardMessages {
        source: ChatId,

        destination: ChatId,

        destination_saved_peer: Option<ChatId>,

        messages: Vec<MessageId>,
    },
    /// Replace this Account's reaction on one Message.
    ReactMessage {
        chat: ChatId,

        message: Box<MessageView>,

        reaction: String,
    },
    /// Change Telegram's pinned state for one Message.
    SetMessagePinned {
        chat: ChatId,

        message: MessageId,

        pinned: bool,
    },
    /// Submit one or more option indices for an open poll or quiz.
    VotePoll {
        chat: ChatId,

        message: Box<MessageView>,

        options: Vec<usize>,
    },
    /// Refresh one specialized Message through its family-specific safe API.
    RefreshSpecialized {
        chat: ChatId,

        message: Box<MessageView>,

        target: SpecializedRefreshTarget,
    },
    /// Change one TODO item's completion state.
    ToggleTodoItem {
        chat: ChatId,

        message: Box<MessageView>,

        item: i32,

        completed: bool,
    },
    /// Append one plain-text item to a TODO list.
    AppendTodoItem {
        chat: ChatId,

        message: Box<MessageView>,

        title: String,
    },
    /// Open an ordinary web destination with the platform browser.
    OpenExternalLink { url: String },
    /// Resolve a supported Telegram username without leaving Intuigram.
    ResolveTelegramUsername { username: String },
    /// Download the Active Message's media bytes.
    DownloadMedia {
        chat: ChatId,

        message: MessageId,

        destination: Option<String>,
    },
    /// Fetch and decode an image for inline presentation without saving it to
    /// Downloads.
    LoadMediaPreview { chat: ChatId, message: MessageId },
    /// Fetch and decode one known peer avatar.
    LoadAvatar { avatar: AvatarRef },
    /// Open or reveal a completed download through the platform adapter.
    OpenDownload { download: DownloadId, reveal: bool },
    /// Shut down adapters and exit.
    Quit,
}

use super::*;
