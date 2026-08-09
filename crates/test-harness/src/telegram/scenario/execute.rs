use intuigram_app::{ChatId, MessageId, MessageView};

use super::{
    ExpectedCommand, HeldSend, HistoryResult, ObservedSend, ScenarioMismatch, TelegramScenario,
};

impl TelegramScenario {
    pub(crate) fn load_history(&mut self, chat: ChatId) -> Result<HistoryResult, ScenarioMismatch> {
        let observed = format!("load history for Chat {}", chat.0);
        let expected = self.next_expected(&observed)?;
        match expected {
            ExpectedCommand::LoadHistory {
                chat: expected_chat,
                status,
                messages,
                pinned_messages,
            } if expected_chat == chat => Ok(HistoryResult::Loaded {
                status,
                messages,
                pinned_messages,
            }),
            ExpectedCommand::FailLoadHistory {
                chat: expected_chat,
                reason,
            } if expected_chat == chat => Ok(HistoryResult::Failed(reason)),
            expected => Err(mismatch(expected, observed)),
        }
    }

    pub fn load_thread(
        &mut self,
        chat: ChatId,
        root: MessageId,
    ) -> Result<Vec<MessageView>, ScenarioMismatch> {
        let observed = format!("load Thread {} in Chat {}", root.0, chat.0);
        let expected = self.next_expected(&observed)?;
        match expected {
            ExpectedCommand::LoadThread {
                chat: expected_chat,
                root: expected_root,
                messages,
            } if expected_chat == chat && expected_root == root => Ok(messages),
            expected => Err(mismatch(expected, observed)),
        }
    }

    pub(crate) fn load_media_preview(
        &mut self,
        chat: ChatId,
        message: MessageId,
    ) -> Result<(), ScenarioMismatch> {
        let observed = format!(
            "load image preview for Message {} in Chat {}",
            message.0, chat.0
        );
        let expected = self.next_expected(&observed)?;
        match expected {
            ExpectedCommand::LoadMediaPreview {
                chat: expected_chat,
                message: expected_message,
            } if expected_chat == chat && expected_message == message => Ok(()),
            expected => Err(mismatch(expected, observed)),
        }
    }

    pub(crate) fn load_avatar(
        &mut self,
        avatar: intuigram_app::AvatarRef,
    ) -> Result<(), ScenarioMismatch> {
        let observed = format!("load avatar {} for peer {}", avatar.id.0, avatar.peer.0);
        let expected = self.next_expected(&observed)?;
        match expected {
            ExpectedCommand::LoadAvatar {
                avatar: expected_avatar,
            } if expected_avatar == avatar => Ok(()),
            expected => Err(mismatch(expected, observed)),
        }
    }

    pub fn read_thread(
        &mut self,
        chat: ChatId,
        root: MessageId,
        max_id: MessageId,
    ) -> Result<(), ScenarioMismatch> {
        let observed = format!(
            "read Thread {} in Chat {} through Message {}",
            root.0, chat.0, max_id.0
        );
        let expected = self.next_expected(&observed)?;
        match expected {
            ExpectedCommand::ReadThread {
                chat: expected_chat,
                root: expected_root,
                max_id: expected_max_id,
            } if expected_chat == chat && expected_root == root && expected_max_id == max_id => {
                Ok(())
            }
            expected => Err(mismatch(expected, observed)),
        }
    }

    pub fn read_history(
        &mut self,
        chat: ChatId,
        max_id: MessageId,
    ) -> Result<bool, ScenarioMismatch> {
        let observed = format!("read Chat {} through Message {}", chat.0, max_id.0);
        let expected = self.next_expected(&observed)?;
        match expected {
            ExpectedCommand::ReadHistory {
                chat: expected_chat,
                max_id: expected_max_id,
                acknowledge,
            } if expected_chat == chat && expected_max_id == max_id => Ok(acknowledge),
            expected => Err(mismatch(expected, observed)),
        }
    }

    pub(crate) fn hold_send(
        &mut self,
        observed_send: ObservedSend,
    ) -> Result<(), ScenarioMismatch> {
        let ObservedSend {
            chat,
            text,
            entities,
            link_preview,
            reply_to,
            thread_root,
            local_id,
        } = observed_send;
        let observed = format!(
            "send {text:?} to Chat {} with link preview {link_preview} replying to {:?} in Thread \
             {:?}",
            chat.0,
            reply_to.map(|message| message.0),
            thread_root.map(|message| message.0)
        );
        let expected = self.next_expected(&observed)?;
        match expected {
            ExpectedCommand::SendText {
                label,
                chat: expected_chat,
                text: expected_text,
                entities: expected_entities,
                link_preview: expected_link_preview,
                reply_to: expected_reply,
                thread_root: expected_thread,
            } if expected_chat == chat
                && expected_text == text
                && expected_entities
                    .as_ref()
                    .is_none_or(|expected| expected == &entities)
                && expected_link_preview.is_none_or(|expected| expected == link_preview)
                && expected_reply == reply_to
                && expected_thread == thread_root =>
            {
                self.held.insert(
                    label,
                    HeldSend {
                        chat,
                        local_id,
                        text,
                        reply_to,
                        thread_root,
                    },
                );
                Ok(())
            }
            expected => Err(mismatch(expected, observed)),
        }
    }

    pub fn edit_message(
        &mut self,
        chat: ChatId,
        message: MessageId,
        text: String,
        entities: Vec<intuigram_app::TextEntity>,
        attachments: Vec<String>,
    ) -> Result<MessageView, ScenarioMismatch> {
        let observed = format!("edit Message {} in Chat {} to {text:?}", message.0, chat.0);
        let expected = self.next_expected(&observed)?;
        match expected {
            ExpectedCommand::EditMessage {
                chat: expected_chat,
                message: expected_message,
                text: expected_text,
                entities: expected_entities,
                attachments: expected_attachments,
                updated,
            } if expected_chat == chat
                && expected_message == message
                && expected_text == text
                && expected_entities
                    .as_ref()
                    .is_none_or(|expected| expected == &entities)
                && expected_attachments
                    .as_ref()
                    .is_none_or(|expected| expected == &attachments) =>
            {
                Ok(updated)
            }
            expected => Err(mismatch(expected, observed)),
        }
    }

    pub fn send_poll(
        &mut self,
        chat: ChatId,
        question: String,
        options: Vec<String>,
        reply_to: Option<MessageId>,
        thread_root: Option<MessageId>,
    ) -> Result<(), ScenarioMismatch> {
        let observed = format!(
            "send poll {question:?} with {options:?} to Chat {} replying to {:?} in Thread {:?}",
            chat.0,
            reply_to.map(|message| message.0),
            thread_root.map(|message| message.0)
        );
        let expected = self.next_expected(&observed)?;
        match expected {
            ExpectedCommand::SendPoll {
                chat: expected_chat,
                question: expected_question,
                options: expected_options,
                reply_to: expected_reply,
                thread_root: expected_thread,
            } if expected_chat == chat
                && expected_question == question
                && expected_options == options
                && expected_reply == reply_to
                && expected_thread == thread_root =>
            {
                Ok(())
            }
            expected => Err(mismatch(expected, observed)),
        }
    }

    pub fn delete_messages(
        &mut self,
        chat: ChatId,
        messages: Vec<MessageId>,
    ) -> Result<(), ScenarioMismatch> {
        let observed = format!(
            "delete Messages {:?} from Chat {}",
            messages.iter().map(|message| message.0).collect::<Vec<_>>(),
            chat.0
        );
        let expected = self.next_expected(&observed)?;
        match expected {
            ExpectedCommand::DeleteMessages {
                chat: expected_chat,
                messages: expected_messages,
            } if expected_chat == chat && expected_messages == messages => Ok(()),
            expected => Err(mismatch(expected, observed)),
        }
    }

    pub fn forward_messages(
        &mut self,
        source: ChatId,
        destination: ChatId,
        messages: Vec<MessageId>,
    ) -> Result<(), ScenarioMismatch> {
        let observed = format!(
            "forward Messages {:?} from Chat {} to Chat {}",
            messages.iter().map(|message| message.0).collect::<Vec<_>>(),
            source.0,
            destination.0
        );
        let expected = self.next_expected(&observed)?;
        match expected {
            ExpectedCommand::ForwardMessages {
                source: expected_source,
                destination: expected_destination,
                messages: expected_messages,
            } if expected_source == source
                && expected_destination == destination
                && expected_messages == messages =>
            {
                Ok(())
            }
            expected => Err(mismatch(expected, observed)),
        }
    }

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

    pub fn reconnect(&mut self) -> Result<(), ScenarioMismatch> {
        let observed = "reconnect".to_owned();
        let expected = self.next_expected(&observed)?;
        if matches!(expected, ExpectedCommand::Reconnect) {
            Ok(())
        } else {
            Err(mismatch(expected, observed))
        }
    }
}

fn mismatch(expected: ExpectedCommand, observed: String) -> ScenarioMismatch {
    ScenarioMismatch {
        expected: expected.describe(),
        observed,
    }
}
