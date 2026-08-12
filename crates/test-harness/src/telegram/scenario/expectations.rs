use super::*;

impl TelegramScenario {
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
            status: None,
            messages,
            pinned_messages,
        });
        self
    }

    /// Expects one foreground Chat refresh carrying fresher header metadata.
    #[must_use]
    pub fn expect_load_history_with_status(
        mut self,
        chat: i64,
        status: impl Into<String>,
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
            status: Some(status.into()),
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
            status: None,
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
            label: None,
            chat: ChatId(chat),
            message: MessageId(message),
        });
        self
    }

    /// Holds one preview request until the scenario completes it explicitly.
    #[must_use]
    pub fn hold_media_preview(mut self, label: impl Into<String>, chat: i64, message: i64) -> Self {
        self.expected.push_back(ExpectedCommand::LoadMediaPreview {
            label: Some(label.into()),
            chat: ChatId(chat),
            message: MessageId(message),
        });
        self
    }

    #[must_use]
    pub fn expect_avatar(mut self, peer: i64) -> Self {
        self.expected.push_back(ExpectedCommand::LoadAvatar {
            avatar: intuigram_lib::AvatarRef {
                peer: ChatId(peer),
                id: intuigram_lib::AvatarId(peer),
            },
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
    pub fn expect_load_topics(
        mut self,
        chat: i64,
        topics: impl IntoIterator<Item = TopicView>,
    ) -> Self {
        self.expected.push_back(ExpectedCommand::LoadTopics {
            chat: ChatId(chat),
            topics: topics.into_iter().collect(),
        });
        self
    }

    #[must_use]
    pub fn expect_load_saved_dialogs(
        mut self,
        chat: i64,
        dialogs: impl IntoIterator<Item = SavedDialogView>,
    ) -> Self {
        self.expected.push_back(ExpectedCommand::LoadSavedDialogs {
            chat: ChatId(chat),
            dialogs: dialogs.into_iter().collect(),
        });
        self
    }

    #[must_use]
    pub fn expect_load_saved_history(
        mut self,
        chat: i64,
        peer: i64,
        messages: impl IntoIterator<Item = MessageView>,
    ) -> Self {
        self.expected.push_back(ExpectedCommand::LoadSavedHistory {
            chat: ChatId(chat),
            peer: ChatId(peer),
            messages: messages.into_iter().collect(),
        });
        self
    }
}
