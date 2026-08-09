impl Client {
    /// Sends a single-choice poll with an ordered set of answers.
    pub async fn send_poll(
        &mut self,
        chat: ChatId,
        question: String,
        options: Vec<String>,
        reply_to: Option<MessageId>,
        thread_root: Option<MessageId>,
        monoforum_peer: Option<ChatId>,
        random_id: i64,
    ) -> Result<()> {
        let peer = self.peers.resolve(chat)?;
        let monoforum_peer = monoforum_peer
            .map(|peer| self.peers.resolve(peer))
            .transpose()?;
        let question = text_with_no_entities(question);
        let answers = options
            .into_iter()
            .enumerate()
            .map(|(index, option)| {
                tl::types::PollAnswer {
                    text: text_with_no_entities(option),
                    option: vec![
                        u8::try_from(index)
                            .expect("the validated Telegram poll has at most ten options"),
                    ],
                    media: None,
                    added_by: None,
                    date: None,
                }
                .into()
            })
            .collect();
        let poll = tl::types::Poll {
            id: 0,
            closed: false,
            public_voters: false,
            multiple_choice: false,
            quiz: false,
            open_answers: false,
            revoting_disabled: false,
            shuffle_answers: false,
            hide_results_until_close: false,
            creator: false,
            subscribers_only: false,
            question,
            answers,
            close_period: None,
            close_date: None,
            countries_iso2: None,
            hash: 0,
        };
        let media = tl::types::InputMediaPoll {
            poll: poll.into(),
            correct_answers: None,
            attached_media: None,
            solution: None,
            solution_entities: None,
            solution_media: None,
        };
        self.connection
            .invoke(&tl::functions::messages::SendMedia {
                silent: false,
                background: false,
                clear_draft: false,
                noforwards: false,
                update_stickersets_order: false,
                invert_media: false,
                allow_paid_floodskip: false,
                peer,
                reply_to: input_reply_to(reply_to, thread_root, monoforum_peer)?,
                media: media.into(),
                message: String::new(),
                random_id,
                reply_markup: None,
                entities: None,
                schedule_date: None,
                schedule_repeat_period: None,
                send_as: None,
                quick_reply_shortcut: None,
                effect: None,
                allow_paid_stars: None,
                suggested_post: None,
            })
            .await
            .context(InvokeSnafu)?;
        Ok(())
    }
}

fn text_with_no_entities(text: String) -> tl::enums::TextWithEntities {
    tl::types::TextWithEntities {
        text,
        entities: Vec::new(),
    }
    .into()
}
use super::*;
