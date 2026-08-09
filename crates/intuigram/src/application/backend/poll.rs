impl Backend {
    pub(super) async fn persist_poll(&mut self, poll: PollPersistence<'_>) -> Result<()> {
        let PollPersistence {
            chat,
            local_id,
            question,
            options,
            reply_to,
            thread_root,
            delivery,
        } = poll;
        let message = MessageView {
            id: local_id,
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
                    remote_id: None,
                }),
                thread_root,
                ..MessageDetails::default()
            },
        };
        self.store
            .save_messages(vec![encode_stored_message(chat, &message)])
            .context(AccountDatabaseSnafu)?
            .await
            .context(AccountDatabaseSnafu)
    }
}

pub(super) struct PollPersistence<'a> {
    pub(super) chat: ChatId,
    pub(super) local_id: MessageId,
    pub(super) question: &'a str,
    pub(super) options: &'a [String],
    pub(super) reply_to: Option<MessageId>,
    pub(super) thread_root: Option<MessageId>,
    pub(super) delivery: DeliveryState,
}
use super::*;
