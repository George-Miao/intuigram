/// Rich and status metadata kept alongside a dense Message row.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MessageDetails {
    /// Stable normalized sender peer used for avatar lookup.
    pub sender_peer: Option<ChatId>,

    /// Local calendar date used for Transcript day boundaries.
    pub date_label: String,

    /// Telegram rich-text entities.
    pub entities: Vec<TextEntity>,

    /// Forward attribution, when present.
    pub forwarded_from: Option<String>,

    /// Aggregate reactions.
    pub reactions: Vec<ReactionView>,

    /// Telegram edit marker.
    pub edited: bool,

    /// Telegram pin marker.
    pub pinned: bool,

    /// View counter for Channels.
    pub views: Option<u32>,

    /// Forward counter.
    pub forwards: Option<u32>,

    /// Reply or comment counter.
    pub replies: Option<u32>,

    /// Media Card or Unsupported Content presentation.
    pub media: Option<MediaCard>,

    /// Telegram grouped-media identifier for an album item.
    pub album_id: Option<i64>,

    /// Service event description, when this is a service Message.
    pub service: Option<String>,

    /// Top Message ID for an ordinary Thread or Channel comments.
    pub thread_root: Option<MessageId>,

    /// Original peer when this Message belongs to a Saved Messages 2.0 dialog.
    pub saved_peer: Option<ChatId>,
}

/// One dense transcript row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageView {
    /// Telegram Message identifier.
    pub id: MessageId,
    /// Sender display name.
    pub sender: String,
    /// Plain-text or semantic fallback body.
    pub body: String,
    /// Compact local-time label supplied by the adapter.
    pub timestamp: String,
    /// Incoming or outgoing presentation.
    pub direction: MessageDirection,
    /// Delivery/read state.
    pub delivery: DeliveryState,
    /// Message being replied to, when any.
    pub reply_to: Option<MessageId>,

    /// Rich content, counters, and semantic Media Card data.
    pub details: MessageDetails,
}

/// Current Draft state for the active Chat.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ComposerView {
    /// Draft text.
    pub text: String,
    /// UTF-8 byte offset of the insertion cursor within `text`.
    pub cursor: usize,
    /// Message targeted by a reply.
    pub reply_to: Option<MessageId>,

    /// Outgoing Message being edited instead of the ordinary Draft.
    pub editing: Option<MessageId>,

    /// Native clipboard or file attachment candidates.
    pub attachments: Vec<AttachmentView>,
}

/// Direction of ordinary Composer cursor movement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComposerMovement {
    /// Move one character toward the start of the Draft.
    Left,
    /// Move one character toward the end of the Draft.
    Right,
    /// Move to the nearest column on the previous visual source line.
    Up,
    /// Move to the nearest column on the next visual source line.
    Down,
}

/// Composer attachment category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttachmentKind {
    /// Photo candidate sent with Telegram photo semantics.
    Photo,

    /// Video candidate sent with Telegram streaming-video semantics.
    Video,

    /// Generic file candidate.
    File,
}

/// Safe display data for an adapter-owned attachment candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachmentView {
    /// Opaque adapter identifier.
    pub id: AttachmentId,

    /// Semantic upload kind.
    pub kind: AttachmentKind,

    /// Filename or clipboard image label.
    pub name: String,
}

/// Active search query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchView {
    /// Search scope selected from the prior focus.
    pub scope: SearchScope,
    /// Query entered so far.
    pub query: String,
}

/// Exact download destination currently being edited.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaveAsView {
    /// User-entered platform path. Existing files are never replaced.
    pub destination: String,
}

/// Exact local attachment path currently being edited.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachmentPathView {
    /// User-entered platform path.
    pub path: String,
}

/// Durable Draft restored before an Account is presented.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DraftView {
    /// Owning Chat.
    pub chat: ChatId,

    /// Thread root, or `None` for the root Chat Draft.
    pub thread_root: Option<MessageId>,

    /// Original peer for a Saved Messages or Channel direct-message Draft.
    pub saved_peer: Option<ChatId>,

    /// Unsent text.
    pub text: String,

    /// Replied-to Message, when any.
    pub reply_to: Option<MessageId>,
}

/// One immediately renderable cached root or Thread history.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryView {
    /// Owning Chat.
    pub chat: ChatId,

    /// Thread root, or `None` for root Chat history.
    pub thread_root: Option<MessageId>,

    /// Original peer when this is one Saved Messages 2.0 dialog history.
    pub saved_peer: Option<ChatId>,

    /// Chronological cached Messages.
    pub messages: Vec<MessageView>,
}

/// One link extracted from the Active Message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkTarget {
    /// Visible linked text.
    pub display: String,

    /// Exact destination that will be opened.
    pub url: String,

    /// Telegram username resolved inside Intuigram, when supported.
    pub telegram_username: Option<String>,

    /// Whether opening requires an explicit destination confirmation.
    pub suspicious: bool,
}

/// A completed download retained without exposing its platform path as an ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownloadView {
    /// Chat containing the downloaded Message.
    pub chat: ChatId,

    /// Opaque adapter-owned file handle.
    pub id: DownloadId,

    /// Exact destination shown to the user.
    pub path: String,

    /// Whether opening is forbidden and only reveal-in-folder is offered.
    pub reveal_only: bool,

    /// Message whose media produced this file.
    pub message: MessageId,

    /// Decoded terminal preview when the downloaded bytes are an image.
    pub preview: Option<super::InlineImage>,
}

/// One automatically loaded image preview associated with its Message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaPreviewView {
    /// Chat containing the Message.
    pub chat: ChatId,

    /// Message containing the image.
    pub message: MessageId,

    /// Bounded decoded image rendered by the terminal adapter.
    pub image: super::InlineImage,
}

/// One decoded peer avatar ready for terminal presentation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AvatarView {
    /// Versioned peer avatar represented by these pixels.
    pub avatar: AvatarRef,

    /// Bounded decoded avatar image.
    pub image: super::InlineImage,
}

/// One image preview whose final Transcript space is reserved while loading.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MediaPreviewLoadView {
    /// Chat containing the Message.
    pub chat: ChatId,

    /// Message containing the image.
    pub message: MessageId,
}
use super::*;
