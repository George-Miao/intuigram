use super::{ChatId, MessageId};

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

    /// Whether Telegram pins this saved dialog.
    pub pinned: bool,

    /// Latest Message identity in the Saved Messages Chat.
    pub top_message: MessageId,
}

/// Cached Saved Messages 2.0 dialog projection for one Account.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedDialogListView {
    /// Owning Saved Messages Chat.
    pub chat: ChatId,

    /// Telegram order, including pinned saved dialogs.
    pub dialogs: Vec<SavedDialogView>,
}

/// Failed Saved Messages projection refresh safe to cross adapter seams.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedDialogLoadFailure {
    /// Owning Saved Messages Chat.
    pub chat: ChatId,

    /// User-facing semantic failure.
    pub reason: String,
}
