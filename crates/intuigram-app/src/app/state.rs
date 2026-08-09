use super::*;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct HistoryKey {
    pub(super) chat: ChatId,
    pub(super) thread: Option<MessageId>,
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
