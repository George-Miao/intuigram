/// Context-sensitive actions shown by every user interface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    /// Exit Intuigram cleanly.
    Quit,
    /// Open exhaustive context help.
    Help,
    /// Move the active item upward.
    MoveUp,
    /// Move the active item downward.
    MoveDown,
    /// Switch to the previous Folder from the Chat list.
    PreviousFolder,
    /// Switch to the next Folder from the Chat list.
    NextFolder,
    /// Open Folder membership for the Active Chat.
    ManageFolders,
    /// Toggle the selected Folder membership for the Active Chat.
    ToggleFolderMembership,
    /// Enter the Active Chat with its Composer focused.
    Open,
    /// Focus the Draft editor.
    Compose,
    /// Send the current Draft.
    Send,
    /// Insert a line break into the current Draft.
    Newline,
    /// Query the native clipboard for text, images, or files.
    Paste,
    /// Replace the Composer with a structured poll editor.
    CreatePoll,
    /// Send the question and options from the poll editor.
    SendPoll,
    /// Reply to the Active Message.
    Reply,
    /// Edit the Active outgoing Message.
    Edit,
    /// Edit the newest eligible outgoing Message from an empty Composer.
    EditPrevious,
    /// Ask for confirmation before deleting the Active Message.
    Delete,
    /// Confirm the pending Message deletion.
    ConfirmDelete,
    /// Choose a destination Chat for the Active Message.
    Forward,
    /// Confirm the selected forward destination.
    ConfirmForward,
    /// Open reactions for the Active Message.
    React,
    /// Apply the selected reaction.
    ConfirmReaction,
    /// Open voting for the Active Message's poll or quiz.
    VotePoll,
    /// Toggle the targeted option in a multiple-choice poll.
    TogglePollChoice,
    /// Submit the selected poll options.
    ConfirmPollVote,
    /// Open the first link in the Active Message.
    OpenLink,
    /// Confirm a suspicious or disguised link destination.
    ConfirmOpenLink,
    /// Download the Active Message's media to the default destination.
    DownloadMedia,
    /// Choose an exact destination for the Active Message's media.
    SaveAs,
    /// Download media to the entered exact destination.
    ConfirmSaveAs,
    /// Open a safe download or reveal launchable content in its folder.
    OpenDownload,
    /// Save the Message currently open for editing.
    SaveEdit,
    /// Open the Active Message's ordinary Thread or Channel comments.
    OpenThread,
    /// Target the newest pinned Message, then cycle toward older pins.
    NavigatePinned,
    /// Pin or unpin the Active cloud Message.
    TogglePin,
    /// Target the previous Message, entering the Transcript from the Composer.
    TargetPreviousMessage,
    /// Target the next Message, returning to the Composer after the newest.
    TargetNextMessage,
    /// Search using the context selected by focus.
    Search,
    /// Cancel the active transient interaction.
    Cancel,
    /// Jump to the oldest loaded Message.
    JumpEarliest,
    /// Jump to the newest loaded Message.
    JumpLatest,
    /// Retry immediately during a reconnect cooldown.
    Reconnect,
}

/// User actions understood by the state owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Intent {
    /// Invoke an action resolved by the effective keymap.
    Action(Action),
    /// Insert text into the Draft or active search query.
    Insert(String),
    /// Remove the final character from the active text field.
    Backspace,
    /// Move the insertion cursor without changing the Draft.
    MoveComposerCursor(ComposerMovement),

    /// Activate a semantic region selected by pointer input.
    Activate(ActivationTarget),

    /// Advance one renderer animation frame while pending work remains.
    Animate,
}

/// Initial synchronized data supplied by an adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bootstrap {
    /// Connectivity represented by this initial data source.
    pub connection: ConnectionState,

    /// Active Account display name.
    pub account_name: String,

    /// Last durable Folder and Chat selection, when one has been saved.
    pub restored_selection: Option<SelectionView>,

    /// Synchronized Telegram Folders.
    pub folders: Vec<FolderView>,
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

    /// A nonfatal Telegram operation failed.
    OperationFailed(String),

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

pub use effects::*;
