use super::*;

/// Sole owner of mutable application state.
pub struct App {
    pub(super) view: View,
    pub(super) all_chats: Vec<ChatView>,
    pub(super) muted_chats: HashSet<ChatId>,
    pub(super) drafts: HashMap<HistoryKey, ComposerView>,
    pub(super) histories: HashMap<HistoryKey, Vec<MessageView>>,
    pub(super) topic_lists: HashMap<ChatId, Vec<TopicView>>,
    pub(super) saved_dialog_lists: HashMap<ChatId, Vec<SavedDialogView>>,
    pub(super) pinned_histories: HashMap<ChatId, Vec<MessageView>>,
    pub(super) projected_pin: bool,
    pub(super) transcript_anchors: HashMap<HistoryKey, MessageId>,
    pub(super) unread_boundaries: HashMap<HistoryKey, MessageId>,
    pub(super) history_loads: HistoryLoads,
    pub(super) media_preview_loads: MediaPreviewLoads,
    pub(super) offline_media: OfflineMedia,
    pub(super) avatar_peers: HashMap<ChatId, AvatarId>,
    pub(super) avatar_loads: AvatarLoads,
    pub(super) small_media_capacity: usize,
    pub(super) next_local_message_id: i64,
    pub(super) pending_drafts: HashMap<MessageId, PendingDraft>,
    pub(super) saved_poll_draft: Option<ComposerView>,
    pub(super) pending_polls: HashMap<MessageId, PendingPoll>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct HistoryKey {
    pub(super) chat: ChatId,

    pub(super) thread: Option<MessageId>,

    /// Saved Messages or Channel direct-message origin.
    pub(super) saved_peer: Option<ChatId>,
}

impl HistoryKey {
    pub(super) const fn root(chat: ChatId) -> Self {
        Self {
            chat,
            thread: None,
            saved_peer: None,
        }
    }

    pub(super) const fn thread(chat: ChatId, root: MessageId) -> Self {
        Self {
            chat,
            thread: Some(root),
            saved_peer: None,
        }
    }

    pub(super) const fn scoped(
        chat: ChatId,
        thread: Option<MessageId>,
        saved_peer: Option<ChatId>,
    ) -> Self {
        Self {
            chat,
            thread,
            saved_peer,
        }
    }

    pub(super) const fn saved(chat: ChatId, peer: ChatId) -> Self {
        Self::scoped(chat, None, Some(peer))
    }
}

#[derive(Clone, Debug)]
pub(super) struct PendingDraft {
    pub(super) history: HistoryKey,
    pub(super) composer: ComposerView,
}

#[derive(Clone, Debug)]
pub(super) struct PendingPoll {
    pub(super) history: HistoryKey,
    pub(super) text: String,
}

impl App {
    pub(super) fn at_latest(&self) -> bool {
        self.view
            .active_message
            .or(self.view.transcript_anchor)
            .is_none_or(|index| Some(index) == self.view.messages.len().checked_sub(1))
    }
}
