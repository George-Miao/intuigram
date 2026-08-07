use std::collections::{HashMap, VecDeque};

use intuigram_app::{Bootstrap, ChatId, MessageId, MessageView, TextEntity};

use super::command::ExpectedCommand;
use super::fixture::AccountFixture;

mod execute;

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
}

pub(crate) enum HistoryResult {
    Loaded {
        messages: Vec<MessageView>,
        pinned_messages: Vec<MessageView>,
    },
    Failed(String),
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
}

impl TelegramScenario {
    #[must_use]
    pub fn new() -> Self {
        Self {
            bootstrap: None,
            expected: VecDeque::new(),
            held: HashMap::new(),
        }
    }

    #[must_use]
    pub fn bootstrap(mut self, account: AccountFixture) -> Self {
        self.bootstrap = Some(account.into_bootstrap());
        self
    }

    #[must_use]
    pub fn expect_load_history(
        mut self,
        chat: i64,
        messages: impl IntoIterator<Item = MessageView>,
    ) -> Self {
        let messages = messages.into_iter().collect::<Vec<_>>();
        let pinned_messages = messages
            .iter()
            .filter(|message| message.details.pinned)
            .cloned()
            .collect();
        self.expected.push_back(ExpectedCommand::LoadHistory {
            chat: ChatId(chat),
            messages,
            pinned_messages,
        });
        self
    }

    #[must_use]
    pub fn expect_load_history_with_pins(
        mut self,
        chat: i64,
        messages: impl IntoIterator<Item = MessageView>,
        pinned_messages: impl IntoIterator<Item = MessageView>,
    ) -> Self {
        self.expected.push_back(ExpectedCommand::LoadHistory {
            chat: ChatId(chat),
            messages: messages.into_iter().collect(),
            pinned_messages: pinned_messages.into_iter().collect(),
        });
        self
    }

    #[must_use]
    pub fn fail_load_history(mut self, chat: i64, reason: impl Into<String>) -> Self {
        self.expected.push_back(ExpectedCommand::FailLoadHistory {
            chat: ChatId(chat),
            reason: reason.into(),
        });
        self
    }

    #[must_use]
    pub fn expect_media_preview(mut self, chat: i64, message: i64) -> Self {
        self.expected.push_back(ExpectedCommand::LoadMediaPreview {
            chat: ChatId(chat),
            message: MessageId(message),
        });
        self
    }

    #[must_use]
    pub fn expect_load_thread(
        mut self,
        chat: i64,
        root: i64,
        messages: impl IntoIterator<Item = MessageView>,
    ) -> Self {
        self.expected.push_back(ExpectedCommand::LoadThread {
            chat: ChatId(chat),
            root: MessageId(root),
            messages: messages.into_iter().collect(),
        });
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
    pub fn hold_send_text(
        self,
        label: impl Into<String>,
        chat: i64,
        text: impl Into<String>,
        reply_to: Option<i64>,
    ) -> Self {
        self.hold_send_in_context(label, chat, text, None, reply_to, None)
    }

    #[must_use]
    pub fn hold_send_in_thread(
        self,
        label: impl Into<String>,
        chat: i64,
        root: i64,
        text: impl Into<String>,
        reply_to: Option<i64>,
    ) -> Self {
        self.hold_send_in_context(label, chat, text, None, reply_to, Some(root))
    }

    #[must_use]
    pub fn hold_send_with_link_preview(
        self,
        label: impl Into<String>,
        chat: i64,
        text: impl Into<String>,
    ) -> Self {
        self.hold_send_in_context(label, chat, text, Some(true), None, None)
    }

    #[must_use]
    pub fn hold_send_rich_text(
        mut self,
        label: impl Into<String>,
        chat: i64,
        text: impl Into<String>,
        entities: Vec<TextEntity>,
    ) -> Self {
        self.expected.push_back(ExpectedCommand::SendText {
            label: label.into(),
            chat: ChatId(chat),
            text: text.into(),
            entities: Some(entities),
            link_preview: None,
            reply_to: None,
            thread_root: None,
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

    fn hold_send_in_context(
        mut self,
        label: impl Into<String>,
        chat: i64,
        text: impl Into<String>,
        link_preview: Option<bool>,
        reply_to: Option<i64>,
        thread_root: Option<i64>,
    ) -> Self {
        self.expected.push_back(ExpectedCommand::SendText {
            label: label.into(),
            chat: ChatId(chat),
            text: text.into(),
            entities: None,
            link_preview,
            reply_to: reply_to.map(MessageId),
            thread_root: thread_root.map(MessageId),
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

    pub fn pending(&self) -> Vec<String> {
        self.expected
            .iter()
            .map(ExpectedCommand::describe)
            .chain(self.held.keys().map(|label| format!("held send {label:?}")))
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
