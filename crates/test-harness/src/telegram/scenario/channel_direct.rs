use super::*;

pub(crate) struct ObservedSavedSend {
    pub(crate) chat: ChatId,
    pub(crate) saved_peer: ChatId,
    pub(crate) text: String,
    pub(crate) reply_to: Option<MessageId>,
    pub(crate) thread_root: Option<MessageId>,
    pub(crate) local_id: MessageId,
}

impl TelegramScenario {
    #[must_use]
    pub fn expect_read_saved_history(mut self, chat: i64, saved_peer: i64, max_id: i64) -> Self {
        self.expected.push_back(ExpectedCommand::ReadSavedHistory {
            chat: ChatId(chat),
            saved_peer: ChatId(saved_peer),
            max_id: MessageId(max_id),
            acknowledge: true,
        });
        self
    }

    #[must_use]
    pub fn hold_send_in_saved_dialog(
        mut self,
        label: impl Into<String>,
        chat: i64,
        saved_peer: i64,
        text: impl Into<String>,
    ) -> Self {
        self.expected.push_back(ExpectedCommand::SendSavedText {
            label: label.into(),
            chat: ChatId(chat),
            saved_peer: ChatId(saved_peer),
            text: text.into(),
            reply_to: None,
            thread_root: None,
        });
        self
    }

    pub(crate) fn read_saved_history(
        &mut self,
        chat: ChatId,
        saved_peer: ChatId,
        max_id: MessageId,
    ) -> Result<bool, ScenarioMismatch> {
        let observed = format!(
            "read peer {} in Chat {} through Message {}",
            saved_peer.0, chat.0, max_id.0
        );
        let expected = self.next_expected(&observed)?;
        match expected {
            ExpectedCommand::ReadSavedHistory {
                chat: expected_chat,
                saved_peer: expected_peer,
                max_id: expected_max_id,
                acknowledge,
            } if expected_chat == chat
                && expected_peer == saved_peer
                && expected_max_id == max_id =>
            {
                Ok(acknowledge)
            }
            expected => Err(mismatch(expected, observed)),
        }
    }

    pub(crate) fn hold_saved_send(
        &mut self,
        observed_send: ObservedSavedSend,
    ) -> Result<(), ScenarioMismatch> {
        let ObservedSavedSend {
            chat,
            saved_peer,
            text,
            reply_to,
            thread_root,
            local_id,
        } = observed_send;
        let observed = format!(
            "send {text:?} to peer {} in Chat {} replying to {:?} in Thread {:?}",
            saved_peer.0,
            chat.0,
            reply_to.map(|message| message.0),
            thread_root.map(|message| message.0)
        );
        let expected = self.next_expected(&observed)?;
        match expected {
            ExpectedCommand::SendSavedText {
                label,
                chat: expected_chat,
                saved_peer: expected_peer,
                text: expected_text,
                reply_to: expected_reply,
                thread_root: expected_thread,
            } if expected_chat == chat
                && expected_peer == saved_peer
                && expected_text == text
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
}

fn mismatch(expected: ExpectedCommand, observed: String) -> ScenarioMismatch {
    ScenarioMismatch {
        expected: expected.describe(),
        observed,
    }
}
