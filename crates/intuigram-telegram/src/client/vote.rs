use super::*;

impl Client {
    /// Submits selected option indices and returns freshly normalized poll
    /// state.
    pub async fn vote_poll(
        &mut self,
        chat: ChatId,
        message: MessageId,
        selected: Vec<usize>,
    ) -> Result<MediaCard> {
        self.vote_poll_with_policy(chat, message, selected, InvocationPolicy::WaitForFlood)
            .await
    }

    /// Submits poll options using the requested mutation invocation policy.
    pub async fn vote_poll_with_policy(
        &mut self,
        chat: ChatId,
        message: MessageId,
        selected: Vec<usize>,
        policy: InvocationPolicy,
    ) -> Result<MediaCard> {
        let media = self.message_media(chat, message).await?;
        let tl::enums::MessageMedia::Poll(media) = media else {
            return PollUnavailableSnafu {
                message_id: message.0,
            }
            .fail();
        };
        let tl::enums::Poll::Poll(poll) = &media.poll;
        if poll.closed {
            return PollUnavailableSnafu {
                message_id: message.0,
            }
            .fail();
        }
        let options = selected
            .into_iter()
            .map(|index| poll_option(poll, message, index))
            .collect::<Result<Vec<_>>>()?;
        let peer = self.peers.resolve(chat)?;
        let msg_id = i32::try_from(message.0).map_err(|_| Error::InvalidMessageId {
            message_id: message.0,
        })?;
        self.invoke_outbound(
            &tl::functions::messages::SendVote {
                peer,
                msg_id,
                options,
            },
            policy,
        )
        .await?;
        let refreshed = self.message_media(chat, message).await?;
        let tl::enums::MessageMedia::Poll(_) = refreshed else {
            return PollUnavailableSnafu {
                message_id: message.0,
            }
            .fail();
        };
        Ok(normalize_media(&refreshed))
    }
}

fn poll_option(poll: &tl::types::Poll, message: MessageId, index: usize) -> Result<Vec<u8>> {
    match poll.answers.get(index) {
        Some(tl::enums::PollAnswer::Answer(answer)) => Ok(answer.option.clone()),
        Some(tl::enums::PollAnswer::InputPollAnswer(_)) | None => PollOptionUnavailableSnafu {
            message_id: message.0,
            option: index,
        }
        .fail(),
    }
}
