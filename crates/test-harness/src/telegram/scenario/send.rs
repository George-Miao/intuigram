use intuigram_lib::{ChatId, MessageId, TextEntity};

use super::{ExpectedCommand, TelegramScenario};

impl TelegramScenario {
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
            attachments: None,
        });
        self
    }

    /// Expects a send whose media came from staged Composer attachments.
    #[must_use]
    pub fn hold_send_with_attachments(
        mut self,
        label: impl Into<String>,
        chat: i64,
        text: impl Into<String>,
        attachments: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.expected.push_back(ExpectedCommand::SendText {
            label: label.into(),
            chat: ChatId(chat),
            text: text.into(),
            entities: None,
            link_preview: None,
            reply_to: None,
            thread_root: None,
            attachments: Some(attachments.into_iter().map(Into::into).collect()),
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
            attachments: None,
        });
        self
    }
}
