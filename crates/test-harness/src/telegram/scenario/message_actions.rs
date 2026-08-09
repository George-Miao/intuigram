use intuigram_app::{ChatId, MessageId, MessageView};

use super::{ExpectedCommand, ScenarioMismatch, TelegramScenario};

impl TelegramScenario {
    pub fn react_message(
        &mut self,
        chat: ChatId,
        message: MessageId,
        reaction: String,
    ) -> Result<MessageView, ScenarioMismatch> {
        let observed = format!(
            "react to Message {} in Chat {} with {reaction:?}",
            message.0, chat.0
        );
        let expected = self.next_expected(&observed)?;
        match expected {
            ExpectedCommand::ReactMessage {
                chat: expected_chat,
                message: expected_message,
                reaction: expected_reaction,
                updated,
            } if expected_chat == chat
                && expected_message == message
                && expected_reaction == reaction =>
            {
                Ok(updated)
            }
            expected => Err(mismatch(expected, observed)),
        }
    }

    pub fn set_message_pinned(
        &mut self,
        chat: ChatId,
        message: MessageId,
        pinned: bool,
    ) -> Result<MessageView, ScenarioMismatch> {
        let observed = format!(
            "set pinned state of Message {} in Chat {} to {pinned}",
            message.0, chat.0
        );
        let expected = self.next_expected(&observed)?;
        match expected {
            ExpectedCommand::SetMessagePinned {
                chat: expected_chat,
                message: expected_message,
                pinned: expected_pinned,
                updated,
            } if expected_chat == chat
                && expected_message == message
                && expected_pinned == pinned =>
            {
                Ok(updated)
            }
            expected => Err(mismatch(expected, observed)),
        }
    }

    pub fn vote_poll(
        &mut self,
        chat: ChatId,
        message: MessageId,
        options: Vec<usize>,
    ) -> Result<MessageView, ScenarioMismatch> {
        let observed = format!(
            "vote for options {options:?} in Message {} of Chat {}",
            message.0, chat.0
        );
        let expected = self.next_expected(&observed)?;
        match expected {
            ExpectedCommand::VotePoll {
                chat: expected_chat,
                message: expected_message,
                options: expected_options,
                updated,
            } if expected_chat == chat
                && expected_message == message
                && expected_options == options =>
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
