use intuigram_lib::{ChatId, MessageId, MessageView, SpecializedRefreshTarget};

use super::{ExpectedCommand, ScenarioMismatch, TelegramScenario};

impl TelegramScenario {
    #[must_use]
    pub fn expect_refresh_specialized(
        mut self,
        chat: i64,
        message: i64,
        target: SpecializedRefreshTarget,
        updated: MessageView,
    ) -> Self {
        self.expected
            .push_back(ExpectedCommand::RefreshSpecialized {
                chat: ChatId(chat),
                message: MessageId(message),
                target,
                updated,
            });
        self
    }

    #[must_use]
    pub fn expect_toggle_todo(
        mut self,
        chat: i64,
        message: i64,
        item: i32,
        completed: bool,
        updated: MessageView,
    ) -> Self {
        self.expected.push_back(ExpectedCommand::ToggleTodoItem {
            chat: ChatId(chat),
            message: MessageId(message),
            item,
            completed,
            updated,
        });
        self
    }

    #[must_use]
    pub fn expect_append_todo(
        mut self,
        chat: i64,
        message: i64,
        title: impl Into<String>,
        updated: MessageView,
    ) -> Self {
        self.expected.push_back(ExpectedCommand::AppendTodoItem {
            chat: ChatId(chat),
            message: MessageId(message),
            title: title.into(),
            updated,
        });
        self
    }

    pub fn refresh_specialized(
        &mut self,
        chat: ChatId,
        message: MessageId,
        target: SpecializedRefreshTarget,
    ) -> Result<MessageView, ScenarioMismatch> {
        let observed = format!(
            "refresh {target:?} in Message {} of Chat {}",
            message.0, chat.0
        );
        let expected = self.next_expected(&observed)?;
        match expected {
            ExpectedCommand::RefreshSpecialized {
                chat: expected_chat,
                message: expected_message,
                target: expected_target,
                updated,
            } if expected_chat == chat
                && expected_message == message
                && expected_target == target =>
            {
                Ok(updated)
            }
            expected => Err(mismatch(expected, observed)),
        }
    }

    pub fn toggle_todo_item(
        &mut self,
        chat: ChatId,
        message: MessageId,
        item: i32,
        completed: bool,
    ) -> Result<MessageView, ScenarioMismatch> {
        let observed = format!(
            "set TODO item {item} in Message {} of Chat {} to {completed}",
            message.0, chat.0
        );
        let expected = self.next_expected(&observed)?;
        match expected {
            ExpectedCommand::ToggleTodoItem {
                chat: expected_chat,
                message: expected_message,
                item: expected_item,
                completed: expected_completed,
                updated,
            } if expected_chat == chat
                && expected_message == message
                && expected_item == item
                && expected_completed == completed =>
            {
                Ok(updated)
            }
            expected => Err(mismatch(expected, observed)),
        }
    }

    pub fn append_todo_item(
        &mut self,
        chat: ChatId,
        message: MessageId,
        title: String,
    ) -> Result<MessageView, ScenarioMismatch> {
        let observed = format!(
            "append TODO item {title:?} to Message {} of Chat {}",
            message.0, chat.0
        );
        let expected = self.next_expected(&observed)?;
        match expected {
            ExpectedCommand::AppendTodoItem {
                chat: expected_chat,
                message: expected_message,
                title: expected_title,
                updated,
            } if expected_chat == chat
                && expected_message == message
                && expected_title == title =>
            {
                Ok(updated)
            }
            expected => Err(mismatch(expected, observed)),
        }
    }
}

fn mismatch(expected: ExpectedCommand, observed: String) -> ScenarioMismatch {
    ScenarioMismatch {
        expected: expected.describe(),
        observed,
    }
}
