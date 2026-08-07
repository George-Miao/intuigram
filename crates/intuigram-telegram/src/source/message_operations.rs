impl Client {
    /// Replaces the text of one existing outgoing Message.
    pub async fn edit_text(
        &mut self,
        chat: ChatId,
        message: MessageId,
        text: String,
        entities: Vec<TextEntity>,
    ) -> Result<()> {
        let peer = self.peers.resolve(chat)?;
        let id = i32::try_from(message.0).map_err(|_| Error::InvalidMessageId {
            message_id: message.0,
        })?;
        let entities = serialize_entities(entities)?;
        self.connection
            .invoke(&tl::functions::messages::EditMessage {
                no_webpage: false,
                invert_media: false,
                peer,
                id,
                message: Some(text),
                media: None,
                reply_markup: None,
                entities,
                schedule_date: None,
                schedule_repeat_period: None,
                quick_reply_shortcut_id: None,
                rich_message: None,
            })
            .await
            .context(InvokeSnafu)?;
        Ok(())
    }

    /// Deletes Messages for both participants where Telegram permits it.
    pub async fn delete_messages(&mut self, chat: ChatId, messages: Vec<MessageId>) -> Result<()> {
        let peer = self.peers.resolve(chat)?;
        let ids = messages
            .into_iter()
            .map(|message| {
                i32::try_from(message.0).map_err(|_| Error::InvalidMessageId {
                    message_id: message.0,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        if let tl::enums::InputPeer::Channel(channel) = peer {
            self.connection
                .invoke(&tl::functions::channels::DeleteMessages {
                    channel: tl::types::InputChannel {
                        channel_id: channel.channel_id,
                        access_hash: channel.access_hash,
                    }
                    .into(),
                    id: ids,
                })
                .await
                .context(InvokeSnafu)?;
        } else {
            self.connection
                .invoke(&tl::functions::messages::DeleteMessages {
                    revoke: true,
                    id: ids,
                })
                .await
                .context(InvokeSnafu)?;
        }
        Ok(())
    }

    /// Forwards one Message to another cloud Chat.
    pub async fn forward_message(
        &mut self,
        source: ChatId,
        destination: ChatId,
        message: MessageId,
        random_id: i64,
    ) -> Result<()> {
        let from_peer = self.peers.resolve(source)?;
        let to_peer = self.peers.resolve(destination)?;
        let id = i32::try_from(message.0).map_err(|_| Error::InvalidMessageId {
            message_id: message.0,
        })?;
        self.connection
            .invoke(&tl::functions::messages::ForwardMessages {
                silent: false,
                background: false,
                with_my_score: false,
                drop_author: false,
                drop_media_captions: false,
                from_peer,
                id: vec![id],
                random_id: vec![random_id],
                to_peer,
                top_msg_id: None,
                reply_to: None,
                schedule_date: None,
                schedule_repeat_period: None,
                send_as: None,
                noforwards: false,
                quick_reply_shortcut: None,
                allow_paid_floodskip: false,
                effect: None,
                video_timestamp: None,
                allow_paid_stars: None,
                suggested_post: None,
            })
            .await
            .context(InvokeSnafu)?;
        Ok(())
    }

    /// Sets this Account's emoji reaction on one Message.
    pub async fn react_message(
        &mut self,
        chat: ChatId,
        message: MessageId,
        reaction: String,
    ) -> Result<()> {
        let peer = self.peers.resolve(chat)?;
        let msg_id = i32::try_from(message.0).map_err(|_| Error::InvalidMessageId {
            message_id: message.0,
        })?;
        self.connection
            .invoke(&tl::functions::messages::SendReaction {
                big: false,
                add_to_recent: true,
                peer,
                msg_id,
                reaction: Some(vec![tl::types::ReactionEmoji { emoticon: reaction }.into()]),
            })
            .await
            .context(InvokeSnafu)?;
        Ok(())
    }
}
use super::*;
