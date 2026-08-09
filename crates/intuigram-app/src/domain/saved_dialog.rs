use super::{ChatId, MessageId};

/// Server Draft attached to one Saved Messages or Channel direct-message
/// dialog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedDialogDraftView {
    /// Unsent text.
    pub text: String,

    /// Direct reply target, when any.
    pub reply_to: Option<MessageId>,
}

/// One original Telegram peer represented inside Saved Messages 2.0.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedDialogView {
    /// Original peer used to filter Saved Messages.
    pub peer: ChatId,

    /// Current peer display title.
    pub title: String,

    /// Latest saved Message fallback.
    pub preview: String,

    /// Latest saved Message timestamp.
    pub timestamp: String,

    /// Number of unread Messages in this dialog.
    pub unread: u32,

    /// Whether Telegram explicitly marks this dialog unread without a count.
    pub unread_mark: bool,

    /// Whether Telegram pins this saved dialog.
    pub pinned: bool,

    /// Latest Message identity in the Saved Messages Chat.
    pub top_message: MessageId,

    /// Server Draft restored when no newer local Draft exists.
    pub draft: Option<SavedDialogDraftView>,
}

/// Cached Saved Messages 2.0 dialog projection for one Account.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedDialogListView {
    /// Owning Saved Messages or monoforum Chat.
    pub chat: ChatId,

    /// Telegram order, including pinned saved dialogs.
    pub dialogs: Vec<SavedDialogView>,
}

/// Failed Saved Messages projection refresh safe to cross adapter seams.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedDialogLoadFailure {
    /// Owning Saved Messages or monoforum Chat.
    pub chat: ChatId,

    /// User-facing semantic failure.
    pub reason: String,
}
