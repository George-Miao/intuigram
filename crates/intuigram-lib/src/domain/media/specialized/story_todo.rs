/// Availability state of a Story shared into a Message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoryStateView {
    /// Telegram included the current Story payload.
    Available,

    /// Telegram retained only a lightweight skipped placeholder.
    Skipped,

    /// The Story was deleted or expired.
    Deleted,

    /// The Message references a Story that has not been fetched.
    Reference,
}

/// Shared Story identity, lifecycle, and text fallback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SharedStoryView {
    /// Peer that published the Story.
    pub peer: super::super::super::ChatId,

    /// Peer-scoped Story identifier.
    pub id: i32,

    /// Current payload availability.
    pub state: StoryStateView,

    /// Story caption when available.
    pub caption: Option<String>,

    /// Local publication date label.
    pub date: String,

    /// Local expiry date label.
    pub expires: String,

    /// Whether the Story was shared through a mention.
    pub via_mention: bool,

    /// Whether Telegram marks it for close friends.
    pub close_friends: bool,

    /// Whether Telegram marks the Story as live.
    pub live: bool,
}

/// One ordered Telegram TODO item and its current completion record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TodoItemView {
    /// Stable item identifier within the Message.
    pub id: i32,

    /// User-facing task title.
    pub title: String,

    /// Whether Telegram reports the item complete.
    pub completed: bool,

    /// Peer that completed the item, when known.
    pub completed_by: Option<super::super::super::ChatId>,

    /// Local completion date label, when known.
    pub completed_date: Option<String>,
}

/// Ordered Telegram TODO list and its collaboration permissions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TodoListView {
    /// List title.
    pub title: String,

    /// Tasks in Telegram-defined order.
    pub items: Vec<TodoItemView>,

    /// Whether other Chat members may append tasks.
    pub others_can_append: bool,

    /// Whether other Chat members may change completion state.
    pub others_can_complete: bool,
}

/// Safe refresh operation supported for one specialized Message family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpecializedRefreshTarget {
    /// Ask Telegram to reveal any newly available paid-media entries.
    PaidMedia,

    /// Fetch the current peer-scoped Story payload.
    Story {
        peer: super::super::super::ChatId,
        id: i32,
    },

    /// Refetch the Message carrying giveaway state or results.
    Giveaway,
}

/// Transient TODO item picker and append editor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TodoListEditorView {
    /// Message carrying the list.
    pub message: super::super::super::MessageId,

    /// Currently selected task.
    pub selected: usize,

    /// Tasks in Telegram-defined order.
    pub items: Vec<TodoItemView>,

    /// Text being appended, when the append editor is active.
    pub append: Option<String>,

    /// Whether the active Account may append tasks.
    pub can_append: bool,

    /// Whether the active Account may change completion state.
    pub can_complete: bool,
}
