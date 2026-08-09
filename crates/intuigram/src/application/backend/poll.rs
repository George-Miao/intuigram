impl Backend {
    pub(super) async fn persist_poll(&mut self, poll: PollPersistence<'_>) -> Result<()> {
        let message = poll_message(poll, poll.local_id, poll.delivery);
        self.store
            .save_messages(vec![encode_stored_message(poll.chat, &message)])
            .context(AccountDatabaseSnafu)?
            .await
            .context(AccountDatabaseSnafu)
    }

    pub(super) async fn acknowledge_poll(
        &mut self,
        poll: PollPersistence<'_>,
        server_id: MessageId,
    ) -> Result<()> {
        let message = poll_message(poll, server_id, DeliveryState::Sent);
        self.store
            .replace_message(
                poll.chat.0,
                poll.local_id.0,
                encode_stored_message(poll.chat, &message),
            )
            .context(AccountDatabaseSnafu)?
            .await
            .context(AccountDatabaseSnafu)
    }
}

fn poll_message(poll: PollPersistence<'_>, id: MessageId, delivery: DeliveryState) -> MessageView {
    let PollPersistence {
        question,
        options,
        reply_to,
        thread_root,
        saved_peer,
        ..
    } = poll;
    MessageView {
        id,
        sender: "You".to_owned(),
        body: String::new(),
        timestamp: "now".to_owned(),
        direction: MessageDirection::Outgoing,
        delivery,
        reply_to,
        details: MessageDetails {
            sender_peer: None,
            media: Some(MediaCard {
                kind: MediaKind::Poll,
                title: "Poll".to_owned(),
                description: question.to_owned(),
                details: Vec::new(),
                poll: Some(PollView {
                    quiz: false,
                    multiple_choice: false,
                    closed: false,
                    total_voters: Some(0),
                    options: options
                        .iter()
                        .map(|option| PollOptionView {
                            text: option.clone(),
                            voters: Some(0),
                            chosen: false,
                            correct: false,
                        })
                        .collect(),
                    solution: None,
                }),
                specialized: None,
                remote_id: None,
            }),
            thread_root,
            saved_peer,
            ..MessageDetails::default()
        },
    }
}

#[derive(Clone, Copy)]
pub(super) struct PollPersistence<'a> {
    pub(super) chat: ChatId,
    pub(super) local_id: MessageId,
    pub(super) question: &'a str,
    pub(super) options: &'a [String],
    pub(super) reply_to: Option<MessageId>,
    pub(super) thread_root: Option<MessageId>,
    pub(super) saved_peer: Option<ChatId>,
    pub(super) delivery: DeliveryState,
}
use super::*;
