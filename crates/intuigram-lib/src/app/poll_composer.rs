impl App {
    pub(super) fn begin_poll(&mut self) {
        if self.view.focus != Focus::Composer || self.view.poll_composer {
            return;
        }
        self.saved_poll_draft = Some(self.view.composer.clone());
        self.view.composer = ComposerView {
            reply_to: self
                .saved_poll_draft
                .as_ref()
                .and_then(|composer| composer.reply_to),
            ..ComposerView::default()
        };
        self.view.poll_composer = true;
        self.view.notice = None;
    }

    pub(super) fn cancel_poll(&mut self) {
        self.view.composer = self.saved_poll_draft.take().unwrap_or_default();
        self.view.poll_composer = false;
        self.view.notice = None;
    }

    pub(super) fn send_poll(&mut self) -> Option<Effect> {
        let (question, options) = match parse_poll(&self.view.composer.text) {
            Some(poll) => poll,
            None => {
                self.view.notice =
                    Some("A poll needs a question and at least two non-empty options".to_owned());
                return None;
            }
        };
        let key = self.active_history_key()?;
        self.next_local_message_id = self.next_local_message_id.saturating_sub(1);
        let local_id = MessageId(self.next_local_message_id);
        let editor_text = self.view.composer.text.clone();
        let reply_to = self.view.composer.reply_to;
        self.pending_polls.insert(
            local_id,
            PendingPoll {
                history: key,
                text: editor_text,
            },
        );
        self.histories.entry(key).or_default().push(pending_poll(
            local_id,
            &question,
            &options,
            reply_to,
            key.thread,
            key.saved_peer,
        ));
        self.view.composer = self.saved_poll_draft.take().unwrap_or_default();
        self.view.poll_composer = false;
        self.view.active_message = None;
        self.view.transcript_anchor = None;
        self.view.focus = Focus::Composer;
        self.view.notice = None;
        self.refresh_active_history();
        Some(Effect::SendPoll {
            chat: key.chat,
            question,
            options,
            reply_to,
            thread_root: key.thread,
            saved_peer: key.saved_peer,
            local_id,
        })
    }
}

fn parse_poll(text: &str) -> Option<(String, Vec<String>)> {
    let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
    let question = lines.next()?.to_owned();
    let options = lines.map(str::to_owned).collect::<Vec<_>>();
    (options.len() >= 2).then_some((question, options))
}

fn pending_poll(
    id: MessageId,
    question: &str,
    options: &[String],
    reply_to: Option<MessageId>,
    thread_root: Option<MessageId>,
    saved_peer: Option<ChatId>,
) -> MessageView {
    MessageView {
        id,
        sender: "You".to_owned(),
        body: String::new(),
        timestamp: "now".to_owned(),
        direction: MessageDirection::Outgoing,
        delivery: DeliveryState::Saving,
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
use super::*;
