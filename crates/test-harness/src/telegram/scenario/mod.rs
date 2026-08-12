use std::collections::{HashMap, VecDeque};

use intuigram_lib::{
    Bootstrap, ChatId, MessageId, MessageView, SavedDialogView, TextEntity, TopicView,
};

use super::command::ExpectedCommand;
use super::fixture::AccountFixture;

mod channel_direct;
mod execute;
mod expectations;
mod location;
mod message_actions;
mod send;
mod specialized;

pub(crate) use channel_direct::ObservedSavedSend;

#[derive(Clone, Debug)]
pub struct HeldSend {
    pub chat: ChatId,
    pub local_id: MessageId,
    pub text: String,
    pub reply_to: Option<MessageId>,
    pub thread_root: Option<MessageId>,
}

pub(crate) struct ObservedSend {
    pub(crate) chat: ChatId,
    pub(crate) text: String,
    pub(crate) entities: Vec<TextEntity>,
    pub(crate) link_preview: bool,
    pub(crate) reply_to: Option<MessageId>,
    pub(crate) thread_root: Option<MessageId>,
    pub(crate) local_id: MessageId,
    pub(crate) attachments: Vec<String>,
}

pub(crate) enum HistoryResult {
    Loaded {
        status: Option<String>,
        messages: Vec<MessageView>,
        pinned_messages: Vec<MessageView>,
    },
    Failed(String),
}

pub(crate) enum MediaPreviewResult {
    Ready,
    Held,
}

#[derive(Debug)]
pub struct ScenarioMismatch {
    pub expected: String,
    pub observed: String,
}

#[derive(Debug)]
pub struct TelegramScenario {
    bootstrap: Option<Bootstrap>,
    expected: VecDeque<ExpectedCommand>,
    held: HashMap<String, HeldSend>,
    held_media_previews: HashMap<String, (ChatId, MessageId)>,
}

impl TelegramScenario {
    #[must_use]
    pub fn new() -> Self {
        Self {
            bootstrap: None,
            expected: VecDeque::new(),
            held: HashMap::new(),
            held_media_previews: HashMap::new(),
        }
    }

    #[must_use]
    pub fn bootstrap(mut self, account: AccountFixture) -> Self {
        self.bootstrap = Some(account.into_bootstrap());
        self
    }

    #[must_use]
    pub fn expect_read_thread(mut self, chat: i64, root: i64, max_id: i64) -> Self {
        self.expected.push_back(ExpectedCommand::ReadThread {
            chat: ChatId(chat),
            root: MessageId(root),
            max_id: MessageId(max_id),
        });
        self
    }

    #[must_use]
    pub fn expect_read_history(mut self, chat: i64, max_id: i64) -> Self {
        self.expected.push_back(ExpectedCommand::ReadHistory {
            chat: ChatId(chat),
            max_id: MessageId(max_id),
            acknowledge: true,
        });
        self
    }

    #[must_use]
    pub fn hold_read_history(mut self, chat: i64, max_id: i64) -> Self {
        self.expected.push_back(ExpectedCommand::ReadHistory {
            chat: ChatId(chat),
            max_id: MessageId(max_id),
            acknowledge: false,
        });
        self
    }

    #[must_use]
    pub fn expect_edit_message(
        mut self,
        chat: i64,
        message: i64,
        text: impl Into<String>,
        updated: MessageView,
    ) -> Self {
        self.expected.push_back(ExpectedCommand::EditMessage {
            chat: ChatId(chat),
            message: MessageId(message),
            text: text.into(),
            entities: None,
            attachments: None,
            updated,
        });
        self
    }

    #[must_use]
    pub fn expect_rich_edit_message(
        mut self,
        chat: i64,
        message: i64,
        text: impl Into<String>,
        entities: Vec<TextEntity>,
        updated: MessageView,
    ) -> Self {
        self.expected.push_back(ExpectedCommand::EditMessage {
            chat: ChatId(chat),
            message: MessageId(message),
            text: text.into(),
            entities: Some(entities),
            attachments: None,
            updated,
        });
        self
    }

    #[must_use]
    pub fn expect_edit_message_with_attachment(
        mut self,
        chat: i64,
        message: i64,
        text: impl Into<String>,
        attachment: impl Into<String>,
        updated: MessageView,
    ) -> Self {
        self.expected.push_back(ExpectedCommand::EditMessage {
            chat: ChatId(chat),
            message: MessageId(message),
            text: text.into(),
            entities: None,
            attachments: Some(vec![attachment.into()]),
            updated,
        });
        self
    }

    #[must_use]
    pub fn expect_delete_messages(
        mut self,
        chat: i64,
        messages: impl IntoIterator<Item = i64>,
    ) -> Self {
        self.expected.push_back(ExpectedCommand::DeleteMessages {
            chat: ChatId(chat),
            messages: messages.into_iter().map(MessageId).collect(),
        });
        self
    }

    #[must_use]
    pub fn expect_forward_message(mut self, source: i64, destination: i64, message: i64) -> Self {
        self.expected.push_back(ExpectedCommand::ForwardMessages {
            source: ChatId(source),
            destination: ChatId(destination),
            messages: vec![MessageId(message)],
        });
        self
    }

    #[must_use]
    pub fn expect_forward_messages(
        mut self,
        source: i64,
        destination: i64,
        messages: impl IntoIterator<Item = i64>,
    ) -> Self {
        self.expected.push_back(ExpectedCommand::ForwardMessages {
            source: ChatId(source),
            destination: ChatId(destination),
            messages: messages.into_iter().map(MessageId).collect(),
        });
        self
    }

    #[must_use]
    pub fn expect_react_message(
        mut self,
        chat: i64,
        message: i64,
        reaction: impl Into<String>,
        updated: MessageView,
    ) -> Self {
        self.expected.push_back(ExpectedCommand::ReactMessage {
            chat: ChatId(chat),
            message: MessageId(message),
            reaction: reaction.into(),
            updated,
        });
        self
    }

    #[must_use]
    pub fn expect_set_message_pinned(
        mut self,
        chat: i64,
        message: i64,
        pinned: bool,
        updated: MessageView,
    ) -> Self {
        self.expected.push_back(ExpectedCommand::SetMessagePinned {
            chat: ChatId(chat),
            message: MessageId(message),
            pinned,
            updated,
        });
        self
    }

    #[must_use]
    pub fn expect_vote_poll(
        mut self,
        chat: i64,
        message: i64,
        options: impl IntoIterator<Item = usize>,
        updated: MessageView,
    ) -> Self {
        self.expected.push_back(ExpectedCommand::VotePoll {
            chat: ChatId(chat),
            message: MessageId(message),
            options: options.into_iter().collect(),
            updated,
        });
        self
    }

    #[must_use]
    pub fn expect_send_poll(
        mut self,
        chat: i64,
        question: impl Into<String>,
        options: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.expected.push_back(ExpectedCommand::SendPoll {
            chat: ChatId(chat),
            question: question.into(),
            options: options.into_iter().map(Into::into).collect(),
            reply_to: None,
            thread_root: None,
        });
        self
    }

    #[must_use]
    pub fn expect_reconnect(mut self) -> Self {
        self.expected.push_back(ExpectedCommand::Reconnect);
        self
    }

    pub fn take_bootstrap(&mut self) -> Option<Bootstrap> {
        self.bootstrap.take()
    }

    pub fn take_held(&mut self, label: &str) -> Option<HeldSend> {
        self.held.remove(label)
    }

    pub fn take_held_media_preview(&mut self, label: &str) -> Option<(ChatId, MessageId)> {
        self.held_media_previews.remove(label)
    }

    pub fn pending(&self) -> Vec<String> {
        self.expected
            .iter()
            .map(ExpectedCommand::describe)
            .chain(self.held.keys().map(|label| format!("held send {label:?}")))
            .chain(
                self.held_media_previews
                    .keys()
                    .map(|label| format!("held media preview {label:?}")),
            )
            .collect()
    }

    fn next_expected(&mut self, observed: &str) -> Result<ExpectedCommand, ScenarioMismatch> {
        self.expected.pop_front().ok_or_else(|| ScenarioMismatch {
            expected: "no further Telegram command".to_owned(),
            observed: observed.to_owned(),
        })
    }
}

impl Default for TelegramScenario {
    fn default() -> Self {
        Self::new()
    }
}
