/// Ordered inputs to the state owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Input {
    /// An action from the active user interface.
    Intent(Intent),
    /// A result from an external adapter.
    Adapter(AdapterEvent),
}

/// Side effects requested from adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
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
    /// Query the native clipboard without blocking terminal input.
    ReadClipboard {
        /// Chat whose Composer requested the paste.
        chat: ChatId,

        /// Thread Composer, when applicable.
        thread_root: Option<MessageId>,
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
    },
    /// Delete one or more Messages from Telegram and durable storage.
    DeleteMessages {
        /// Chat containing the Messages.
        chat: ChatId,

        /// Telegram Message IDs to delete.
        messages: Vec<MessageId>,
    },
    /// Forward one Message between cloud Chats.
    ForwardMessage {
        /// Source Chat containing the Message.
        source: ChatId,

        /// Destination Chat selected by the user.
        destination: ChatId,

        /// Telegram Message to forward.
        message: MessageId,
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

/// Immutable data rendered by a user interface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct View {
    /// Current Telegram connectivity.
    pub connection: ConnectionState,

    /// Active Account display name.
    pub account_name: String,

    /// Synchronized Telegram Folders.
    pub folders: Vec<FolderView>,

    /// Active Folder index.
    pub active_folder: usize,

    /// Chats in the active Folder.
    pub chats: Vec<ChatView>,

    /// Active Chat index.
    pub active_chat: Option<usize>,

    /// Loaded messages for the active Chat.
    pub messages: Vec<MessageView>,

    /// Active Message index.
    pub active_message: Option<usize>,

    /// Active ordinary Thread or Channel comments root.
    pub active_thread: Option<MessageId>,

    /// Message index anchoring the Transcript when no Message is active.
    pub transcript_anchor: Option<usize>,

    /// Region receiving navigation and editing input.
    pub focus: Focus,

    /// Current Draft.
    pub composer: ComposerView,

    /// Active search, when open.
    pub search: Option<SearchView>,

    /// Whether unseen messages arrived while reading older history.
    pub has_newer_messages: bool,

    /// Whether exhaustive context help is open.
    pub help_open: bool,

    /// Selected index in the Folder membership overlay, when open.
    pub folder_picker: Option<usize>,

    /// Message awaiting explicit destructive-action confirmation.
    pub delete_confirmation: Option<MessageId>,

    /// Selected destination index in the forward Chat picker.
    pub forward_picker: Option<usize>,

    /// Compact reaction picker for the Active Message.
    pub reaction_picker: Option<ReactionPickerView>,

    /// Poll or quiz option picker for the Active Message.
    pub poll_vote: Option<PollVoteView>,

    /// Suspicious link awaiting explicit destination confirmation.
    pub link_confirmation: Option<LinkTarget>,

    /// Completed downloads retained by Chat and Message for inline previews.
    pub downloads: Vec<DownloadView>,

    /// Automatically loaded inline image previews for visible histories.
    pub media_previews: Vec<MediaPreviewView>,

    /// Whether the Composer contains a poll question and answer choices.
    pub poll_composer: bool,

    /// Latest nonfatal adapter notice.
    pub notice: Option<String>,

    /// Actions valid in the current context.
    pub actions: Vec<Action>,
}

/// One state transition observed by the composition root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Update {
    /// Immutable view after applying the input.
    pub view: View,
    /// Optional external work requested by the transition.
    pub effect: Option<Effect>,
}
use super::*;
