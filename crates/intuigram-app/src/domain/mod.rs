mod account;
mod formatting;
mod links;
mod media;
mod message;

pub use account::*;
pub(crate) use formatting::format_markdown;
pub(crate) use links::active_link;
pub use media::*;
pub use message::*;

/// Stable identifier for a Telegram chat.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ChatId(pub i64);

/// Stable identifier for a Telegram message.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MessageId(pub i64);

/// Opaque attachment candidate owned by the composition adapter.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AttachmentId(pub u64);

/// Opaque downloaded-file handle owned by the composition adapter.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DownloadId(pub u64);

/// Current Telegram connection state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionState {
    /// The account is synchronized over a live connection.
    Connected,
    /// A connection attempt is in progress.
    Connecting,
    /// Automatic reconnection is waiting for its backoff deadline.
    ReconnectCooldown,
}

/// Loading state for the history currently presented in the Transcript.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChatLoadingState {
    /// No foreground history work is pending.
    Idle,

    /// The Transcript is empty while its first history window loads.
    Fresh,

    /// Cached Messages remain visible while Telegram refreshes them.
    Updating,
}

/// Current interaction target within the TUI hierarchy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Focus {
    /// Chat list.
    Chats,
    /// Active Chat transcript.
    Transcript,
    /// Message Draft editor.
    Composer,
    /// Context-sensitive search field.
    Search,
}

/// Scope selected when context-sensitive search opens.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchScope {
    /// Search the active Chat.
    Chat,
    /// Search every synchronized Chat in the active Account.
    Account,
}

/// One Telegram Folder presented in the bottom Folder strip.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FolderView {
    /// Telegram Folder identifier.
    pub id: i32,
    /// Display name.
    pub title: String,
    /// Aggregate unread count.
    pub unread: u32,
}

/// Last durable navigation target for one Account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionView {
    /// Telegram Folder ID selected in the Chat list.
    pub folder: i32,

    /// Selected Chat, or `None` when no Chat is selected.
    pub chat: Option<ChatId>,

    /// Message anchoring the restored Transcript viewport.
    pub message: Option<MessageId>,
}

/// One Account-local Transcript position safe to persist through adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TranscriptAnchorView {
    /// Owning Chat.
    pub chat: ChatId,

    /// Thread root, or `None` for root Chat history.
    pub thread: Option<MessageId>,

    /// Message anchoring the viewport.
    pub message: MessageId,
}

/// Semantic target selected by a pointing device.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationTarget {
    /// Switch to a Telegram Folder and return interaction to the Chat list.
    Folder(i32),

    /// Select a Chat while retaining Chat-list interaction.
    Chat(ChatId),

    /// Select a Message and descend to Transcript interaction.
    Message(MessageId),

    /// Focus the active Chat's Composer.
    Composer,
}

/// Telegram cloud Chat category normalized away from TL constructors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChatKind {
    /// The active Account's Saved Messages Chat.
    SavedMessages,

    /// A human Private Chat.
    Private,

    /// An ordinary bot Private Chat.
    Bot,

    /// A legacy basic group.
    BasicGroup,

    /// A modern group without gigagroup restrictions.
    Supergroup,

    /// A group where only administrators may post.
    Gigagroup,

    /// A broadcast Channel.
    Channel,

    /// Telegram exposed an identity that cannot currently be accessed.
    Inaccessible,
}

/// Dense summary of a synchronized Chat.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatView {
    /// Telegram Chat identifier.
    pub id: ChatId,
    /// Display name.
    pub title: String,
    /// Compact last-message preview.
    pub preview: String,

    /// Idle Active-Chat metadata normalized for display.
    pub status: String,

    /// Unread message count.
    pub unread: u32,
    /// Whether Telegram pins this Chat.
    pub pinned: bool,

    /// Whether current Telegram rights permit pinning Messages in this Chat.
    pub can_pin_messages: bool,

    /// Normalized cloud Chat category.
    pub kind: ChatKind,

    /// Folder identifiers containing this Chat. `0` is All and `-1` Archive.
    pub folders: Vec<i32>,
}

/// Sender direction for transcript styling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageDirection {
    /// Message received from another peer.
    Incoming,
    /// Message sent by the active Account.
    Outgoing,
}

/// Delivery state kept separate from local durability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryState {
    /// Locally durable and waiting for Telegram acknowledgement.
    Pending,
    /// Telegram accepted the message.
    Sent,
    /// Telegram reports that the recipient read the message.
    Read,
    /// The send reached a terminal error.
    Failed,
}

/// Rich-text semantic recognized by Intuigram.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextEntityKind {
    /// Bold emphasis.
    Bold,

    /// Italic emphasis.
    Italic,

    /// Underlined text.
    Underline,

    /// Struck text.
    Strike,

    /// Inline code.
    Code,

    /// Preformatted code block with an optional language.
    Pre { language: Option<String> },

    /// Spoiler text.
    Spoiler,

    /// Ordinary URL present in the body.
    Url,

    /// Display text pointing at a separate URL.
    TextUrl { url: String },

    /// Mention, hashtag, cashtag, bot command, email, or phone token.
    Semantic,

    /// Custom emoji document.
    CustomEmoji { document_id: i64 },
}

/// One UTF-16-indexed Telegram rich-text entity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextEntity {
    /// UTF-16 code-unit offset.
    pub offset: usize,

    /// UTF-16 code-unit length.
    pub length: usize,

    /// Entity semantic.
    pub kind: TextEntityKind,
}

/// One aggregate reaction shown on a Message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReactionView {
    /// Emoji or semantic label.
    pub label: String,

    /// Aggregate reaction count.
    pub count: u32,

    /// Whether the active Account selected it.
    pub chosen: bool,
}

/// Transient reaction choices for the Active Message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReactionPickerView {
    /// Zero-based active reaction option.
    pub selected: usize,

    /// Emoji reactions offered by the current compact picker.
    pub options: Vec<String>,
}
