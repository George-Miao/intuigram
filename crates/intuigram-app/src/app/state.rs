use super::*;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct HistoryKey {
    pub(super) chat: ChatId,
    pub(super) thread: Option<MessageId>,
    /// Saved Messages 2.0 origin. Never set together with `thread`.
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

    pub(super) const fn from_thread(chat: ChatId, thread: Option<MessageId>) -> Self {
        Self {
            chat,
            thread,
            saved_peer: None,
        }
    }

    pub(super) const fn saved(chat: ChatId, peer: ChatId) -> Self {
        Self {
            chat,
            thread: None,
            saved_peer: Some(peer),
        }
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
