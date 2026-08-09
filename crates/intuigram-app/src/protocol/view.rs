use super::*;

/// Immutable data rendered by a user interface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct View {
    /// Current Telegram connectivity.
    pub connection: ConnectionState,

    /// Active Account display name.
    pub account_name: String,

    /// Stable Account-scoped notification replacement identity.
    pub notification_identity: String,

    /// Registered Accounts available to the Account picker.
    pub accounts: Vec<AccountView>,

    /// Synchronized Telegram Folders.
    pub folders: Vec<FolderView>,

    /// Editable metadata for synchronized custom Folders.
    pub folder_details: Vec<FolderDetailsView>,

    /// Active Folder index.
    pub active_folder: usize,

    /// Chats in the active Folder.
    pub chats: Vec<ChatView>,

    /// Chats whose original media is retained outside ordinary cache eviction.
    pub offline_chats: Vec<ChatId>,

    /// Active Chat index.
    pub active_chat: Option<usize>,

    /// Topics for the Active Chat, in Telegram order.
    pub topics: Vec<TopicView>,

    /// Selected Topic while the Topic list is active.
    pub active_topic: Option<usize>,

    /// Whether the Active Chat's Topic projection is being refreshed.
    pub topics_loading: bool,

    /// Original-peer dialogs for the Active Saved Messages Chat.
    pub saved_dialogs: Vec<SavedDialogView>,

    /// Selected Saved Messages dialog while its list is active.
    pub active_saved_dialog: Option<usize>,

    /// Original peer whose saved history is currently open.
    pub active_saved_peer: Option<ChatId>,

    /// Whether the Saved Messages dialog projection is refreshing.
    pub saved_dialogs_loading: bool,

    /// Direction of the most recent Chat-list movement, used for viewport
    /// anchoring.
    pub chat_scroll_direction: ScrollDirection,

    /// Loaded messages for the active Chat.
    pub messages: Vec<MessageView>,

    /// Root Chat history retained beside an ordinary Thread in wide layouts.
    pub parent_messages: Vec<MessageView>,

    /// Foreground loading state for the history presented in the Transcript.
    pub chat_loading: ChatLoadingState,

    /// Pinned Messages in the Active Chat, independently of visible history.
    pub pinned_messages: Vec<MessageView>,

    /// Active Message index.
    pub active_message: Option<usize>,

    /// Explicit Message Selection in the active history.
    pub selected_messages: Vec<MessageId>,

    /// Active ordinary Thread or Channel comments root.
    pub active_thread: Option<MessageId>,

    /// Message index anchoring the Transcript when no Message is active.
    pub transcript_anchor: Option<usize>,

    /// First unread incoming Message in the active root Chat history.
    pub unread_boundary: Option<MessageId>,

    /// Region receiving navigation and editing input.
    pub focus: Focus,

    /// Current Draft.
    pub composer: ComposerView,

    /// Active search, when open.
    pub search: Option<SearchView>,

    /// Exact download destination editor, when open.
    pub save_as: Option<SaveAsView>,

    /// Exact local attachment path editor, when open.
    pub attachment_path: Option<AttachmentPathView>,

    /// Whether unseen messages arrived while reading older history.
    pub has_newer_messages: bool,

    /// Whether exhaustive context help is open.
    pub help_open: bool,

    /// Context actions grouped for the current interaction target.
    pub action_menu: Option<ActionMenuView>,

    /// Selected index in the Folder membership overlay, when open.
    pub folder_picker: Option<usize>,

    /// Folder lifecycle management overlay, when open.
    pub folder_manager: Option<FolderManagerView>,

    /// Rich-media Composer surface, when open.
    pub rich_media: Option<RichMediaComposerView>,

    /// Server-owned Scheduled Message management surface.
    pub scheduled: Option<ScheduledManagerView>,

    /// Selected index in the Account picker; the final index is Add Account.
    pub account_picker: Option<usize>,

    /// Destructive Account operation awaiting explicit confirmation.
    pub account_confirmation: Option<AccountConfirmationView>,

    /// Messages awaiting explicit destructive-action confirmation.
    pub delete_confirmation: Option<Vec<MessageId>>,

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

    /// Decoded avatars retained by normalized peer identity.
    pub avatars: Vec<AvatarView>,

    /// Image previews with final Transcript space currently reserved.
    pub media_preview_loads: Vec<MediaPreviewLoadView>,

    /// Whether the Composer contains a poll question and answer choices.
    pub poll_composer: bool,

    /// Latest nonfatal adapter notice.
    pub notice: Option<String>,

    /// Monotonic wrapping frame used by renderer-owned effort animations.
    pub animation_frame: u8,

    /// Actions valid in the current context.
    pub actions: Vec<Action>,
}

impl View {
    /// Reports whether the renderer needs another animation frame.
    #[must_use]
    pub fn has_pending_effort(&self) -> bool {
        self.connection == ConnectionState::Connecting
            || self.chat_loading != ChatLoadingState::Idle
            || self.topics_loading
            || self.saved_dialogs_loading
            || self
                .folder_manager
                .as_ref()
                .is_some_and(|manager| manager.pending)
            || self
                .rich_media
                .as_ref()
                .is_some_and(|composer| composer.pending)
            || self
                .scheduled
                .as_ref()
                .is_some_and(|manager| manager.pending)
            || !self.media_preview_loads.is_empty()
            || self
                .messages
                .iter()
                .any(|message| message.delivery == DeliveryState::Pending)
    }
}

/// One state transition observed by the composition root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Update {
    /// Immutable view after applying the input.
    pub view: View,

    /// Optional external work requested by the transition.
    pub effect: Option<Effect>,
}
