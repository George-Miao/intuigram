/// Side effects requested from adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    /// Ask the composition root to change the active Account lifecycle.
    AccountLifecycle {
        /// Requested switch, authorization, or removal operation.
        request: AccountLifecycle,
    },
    /// Create, edit, reorder, share, or delete a Telegram Folder.
    FolderOperation {
        /// Complete typed operation.
        operation: FolderOperation,
    },
    /// Refresh the authoritative Folder and Chat-membership projection.
    RefreshFolders,
    /// Load recent or saved Telegram media for Composer selection.
    BrowseRichMedia {
        /// Telegram-owned library.
        kind: RichMediaLibraryKind,
    },
    /// Send a previously browsed Telegram media item.
    SendLibraryMedia {
        /// Destination Chat.
        chat: ChatId,

        /// Opaque adapter-owned library item.
        item: RichMediaItemId,

        /// Optimistic local Message identity.
        local_id: MessageId,

        /// Direct reply target.
        reply_to: Option<MessageId>,

        /// Active Thread root.
        thread_root: Option<MessageId>,
    },
    /// Read and upload one exact local path with explicit media semantics.
    SendRichMediaFile {
        /// Destination Chat.
        chat: ChatId,

        /// Exact local path without shell expansion.
        path: String,

        /// Explicit Telegram upload presentation.
        kind: RichMediaUploadKind,

        /// Optimistic local Message identity.
        local_id: MessageId,

        /// Direct reply target.
        reply_to: Option<MessageId>,

        /// Active Thread root.
        thread_root: Option<MessageId>,
    },
    /// Capture and send a voice or circular video note.
    RecordRichMedia {
        /// Destination Chat.
        chat: ChatId,

        /// Voice or circular-video capture kind.
        kind: RichMediaUploadKind,

        /// Capture duration in seconds.
        seconds: u32,

        /// Platform capture device understood by ffmpeg.
        device: String,

        /// Optimistic local Message identity.
        local_id: MessageId,

        /// Direct reply target.
        reply_to: Option<MessageId>,

        /// Active Thread root.
        thread_root: Option<MessageId>,
    },
    /// Send a Telegram contact card from Composer fields.
    SendContact {
        /// Destination Chat.
        chat: ChatId,

        /// Telegram-compatible telephone number.
        phone: String,

        /// Contact first name.
        first_name: String,

        /// Optional contact last name.
        last_name: String,

        /// Optimistic local Message identity.
        local_id: MessageId,

        /// Direct reply target.
        reply_to: Option<MessageId>,

        /// Active Thread root.
        thread_root: Option<MessageId>,
    },
    /// Load server-owned Scheduled Message history independently of Transcript
    /// history.
    LoadScheduledMessages {
        /// Owning Chat.
        chat: ChatId,
    },
    /// Apply one typed Scheduled Message mutation.
    ScheduledOperation {
        /// Owning Chat.
        chat: ChatId,
        /// Complete mutation.
        request: ScheduledRequest,
    },
    /// Alert the user about an incoming Message outside the visibly read Chat.
    Notify {
        /// Stable Account-scoped replacement identity.
        identity: String,

        /// Chat receiving the incoming Message.
        chat: ChatId,
    },
    /// Start a connection attempt immediately.
    Reconnect,
    /// Add or remove the Active Chat from one Telegram Folder.
    SetChatFolder {
        /// Chat whose membership changes.
        chat: ChatId,

        /// Custom Folder ID, or `-1` for Archive.
        folder: i32,

        /// Whether the Chat should belong to the Folder.
        included: bool,
    },
    /// Load recent history for the selected Chat.
    LoadChat {
        /// Chat selected by the user.
        chat: ChatId,

        /// Navigation state to persist with this foreground request.
        selection: Option<SelectionView>,

        /// Complete Account-local Transcript positions to persist atomically.
        transcript_anchors: Vec<TranscriptAnchorView>,
    },
    /// Persist navigation when no Chat load is needed.
    SaveSelection {
        /// Selected Telegram Folder ID.
        folder: i32,

        /// Selected Chat, or `None` when the Folder has no active Chat.
        chat: Option<ChatId>,

        /// Message anchoring the Transcript viewport.
        message: Option<MessageId>,

        /// Complete Account-local Transcript positions to persist atomically.
        transcript_anchors: Vec<TranscriptAnchorView>,
    },
    /// Load an ordinary Message Thread or Channel comments.
    LoadThread {
        /// Parent Chat.
        chat: ChatId,

        /// Thread root Message.
        root: MessageId,
    },
    /// Advance read state for an ordinary Thread or Channel comments.
    ReadThread {
        /// Parent Chat.
        chat: ChatId,

        /// Thread root Message.
        root: MessageId,

        /// Highest visible incoming Message.
        max_id: MessageId,
    },
    /// Advance read state for visible root Chat history.
    ReadHistory {
        /// Chat whose root history is visible.
        chat: ChatId,

        /// Highest visible incoming Message.
        max_id: MessageId,
    },
    /// Query the native clipboard without blocking terminal input.
    ReadClipboard {
        /// Chat whose Composer requested the paste.
        chat: ChatId,

        /// Thread Composer, when applicable.
        thread_root: Option<MessageId>,
    },
    /// Ask the configured platform picker for a local attachment path.
    PickAttachment {
        /// Chat whose Composer requested the attachment.
        chat: ChatId,

        /// Thread Composer, when applicable.
        thread_root: Option<MessageId>,
    },
    /// Validate and retain one exact local attachment path.
    SelectAttachment {
        /// Chat whose Composer owns the attachment.
        chat: ChatId,

        /// Thread Composer, when applicable.
        thread_root: Option<MessageId>,

        /// Exact user-entered platform path.
        path: String,
    },
    /// Persist a changed Draft before any saved indication is emitted.
    SaveDraft {
        /// Owning Chat.
        chat: ChatId,

        /// Thread Composer, when applicable.
        thread_root: Option<MessageId>,

        /// Complete Draft text.
        text: String,

        /// Reply target.
        reply_to: Option<MessageId>,
    },
    /// Send one text Message, optionally as a reply.
    SendMessage {
        /// Destination Chat.
        chat: ChatId,
        /// Draft contents.
        text: String,

        /// Telegram entities extracted from supported composer markup.
        entities: Vec<TextEntity>,

        /// Whether Telegram should generate a webpage preview for detected
        /// links.
        link_preview: bool,

        /// Replied-to Message.
        reply_to: Option<MessageId>,

        /// Active Thread root, when sending inside a Thread.
        thread_root: Option<MessageId>,

        /// Adapter-owned attachments to upload.
        attachments: Vec<AttachmentId>,

        /// Optimistic local Message to acknowledge or fail.
        local_id: MessageId,
    },
    /// Send a Telegram poll with at least two choices.
    SendPoll {
        /// Destination Chat.
        chat: ChatId,

        /// Poll question.
        question: String,

        /// Ordered answer choices.
        options: Vec<String>,

        /// Replied-to Message.
        reply_to: Option<MessageId>,

        /// Active Thread root, when sending inside a Thread.
        thread_root: Option<MessageId>,

        /// Optimistic local Message to acknowledge or fail.
        local_id: MessageId,
    },
    /// Replace the text of one outgoing Message.
    EditMessage {
        /// Chat containing the Message.
        chat: ChatId,

        /// Complete normalized replacement retained for persistence and UI.
        message: Box<MessageView>,

        /// Exact composer contents restored if Telegram rejects the edit.
        draft_text: String,

        /// Adapter-owned replacement media, when the edit changes a photo.
        attachments: Vec<AttachmentId>,

        /// Safe attachment presentation restored if Telegram rejects the edit.
        draft_attachments: Vec<AttachmentView>,
    },
    /// Delete one or more Messages from Telegram and durable storage.
    DeleteMessages {
        /// Chat containing the Messages.
        chat: ChatId,

        /// Telegram Message IDs to delete.
        messages: Vec<MessageId>,
    },
    /// Forward one or more Messages between cloud Chats.
    ForwardMessages {
        /// Source Chat containing the Message.
        source: ChatId,

        /// Destination Chat selected by the user.
        destination: ChatId,

        /// Telegram Messages to forward in Transcript order.
        messages: Vec<MessageId>,
    },
    /// Replace this Account's reaction on one Message.
    ReactMessage {
        /// Chat containing the Message.
        chat: ChatId,

        /// Complete normalized Message after the reaction change.
        message: Box<MessageView>,

        /// Emoji sent through Telegram's reaction API.
        reaction: String,
    },
    /// Change Telegram's pinned state for one Message.
    SetMessagePinned {
        /// Chat containing the Message.
        chat: ChatId,

        /// Telegram Message to change.
        message: MessageId,

        /// New pinned state.
        pinned: bool,
    },
    /// Submit one or more option indices for an open poll or quiz.
    VotePoll {
        /// Chat containing the poll.
        chat: ChatId,

        /// Complete normalized Message after the local vote selection.
        message: Box<MessageView>,

        /// Telegram-ordered option indices selected by the Account.
        options: Vec<usize>,
    },
    /// Open an ordinary web destination with the platform browser.
    OpenExternalLink {
        /// Exact destination, already confirmed when suspicious.
        url: String,
    },
    /// Resolve a supported Telegram username without leaving Intuigram.
    ResolveTelegramUsername {
        /// Telegram username without `@`.
        username: String,
    },
    /// Download the Active Message's media bytes.
    DownloadMedia {
        /// Chat containing the media Message.
        chat: ChatId,

        /// Message containing the downloadable media.
        message: MessageId,

        /// Exact user-selected path, or `None` for the configured Downloads
        /// directory.
        destination: Option<String>,
    },
    /// Fetch and decode an image for inline presentation without saving it to
    /// Downloads.
    LoadMediaPreview {
        /// Chat containing the media Message.
        chat: ChatId,

        /// Message containing the image.
        message: MessageId,
    },
    /// Open or reveal a completed download through the platform adapter.
    OpenDownload {
        /// Opaque adapter-owned download handle.
        download: DownloadId,

        /// Reveal in a folder instead of launching the associated application.
        reveal: bool,
    },
    /// Shut down adapters and exit.
    Quit,
}

use super::*;
