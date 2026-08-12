use grammers_tl_types::Serializable as _;

/// Replacement contents for one existing outgoing Message.
pub struct MessageEdit {
    /// Chat containing the Message.
    pub chat: ChatId,

    /// Message to replace.
    pub message: MessageId,

    /// New text or media caption.
    pub text: String,

    /// Caption entities using UTF-16 offsets.
    pub entities: Vec<TextEntity>,

    /// New media and its upload identifier, when replacing media.
    pub upload: Option<(Upload, i64)>,
}

impl Client {
    /// Replaces the text, caption, or media of one existing outgoing Message.
    pub async fn edit_message(&mut self, request: MessageEdit) -> Result<()> {
        self.edit_message_with_policy(request, InvocationPolicy::WaitForFlood)
            .await
    }

    /// Replaces one outgoing Message using the requested invocation policy.
    pub async fn edit_message_with_policy(
        &mut self,
        request: MessageEdit,
        policy: InvocationPolicy,
    ) -> Result<()> {
        let MessageEdit {
            chat,
            message,
            text,
            entities,
            upload,
        } = request;
        let peer = self.peers.resolve(chat)?;
        let id = i32::try_from(message.0).map_err(|_| Error::InvalidMessageId {
            message_id: message.0,
        })?;
        let entities = serialize_entities(entities)?;
        let media = match upload {
            Some((upload, file_id)) => Some(
                self.upload_media_with_policy(upload, file_id, policy)
                    .await?,
            ),
            None => None,
        };
        self.invoke_outbound(
            &tl::functions::messages::EditMessage {
                no_webpage: false,
                invert_media: false,
                peer,
                id,
                message: Some(text),
                media,
                reply_markup: None,
                entities,
                schedule_date: None,
                schedule_repeat_period: None,
                quick_reply_shortcut_id: None,
                rich_message: None,
            },
            policy,
        )
        .await?;
        Ok(())
    }

    /// Deletes Messages for both participants where Telegram permits it.
    pub async fn delete_messages(&mut self, chat: ChatId, messages: Vec<MessageId>) -> Result<()> {
        self.delete_messages_with_policy(chat, messages, InvocationPolicy::WaitForFlood)
            .await
    }

    /// Deletes Messages using the requested invocation policy.
    pub async fn delete_messages_with_policy(
        &mut self,
        chat: ChatId,
        messages: Vec<MessageId>,
        policy: InvocationPolicy,
    ) -> Result<()> {
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
            self.invoke_outbound(
                &tl::functions::channels::DeleteMessages {
                    channel: tl::types::InputChannel {
                        channel_id: channel.channel_id,
                        access_hash: channel.access_hash,
                    }
                    .into(),
                    id: ids,
                },
                policy,
            )
            .await?;
        } else {
            self.invoke_outbound(
                &tl::functions::messages::DeleteMessages {
                    revoke: true,
                    id: ids,
                },
                policy,
            )
            .await?;
        }
        Ok(())
    }

    /// Forwards one or more Messages to another cloud Chat.
    pub async fn forward_messages(
        &mut self,
        source: ChatId,
        destination: ChatId,
        destination_monoforum_peer: Option<ChatId>,
        messages: Vec<MessageId>,
        first_random_id: i64,
    ) -> Result<()> {
        self.forward_messages_with_policy(
            source,
            destination,
            destination_monoforum_peer,
            messages,
            first_random_id,
            InvocationPolicy::WaitForFlood,
        )
        .await
    }

    /// Forwards Messages using the requested invocation policy.
    pub async fn forward_messages_with_policy(
        &mut self,
        source: ChatId,
        destination: ChatId,
        destination_monoforum_peer: Option<ChatId>,
        messages: Vec<MessageId>,
        first_random_id: i64,
        policy: InvocationPolicy,
    ) -> Result<()> {
        let from_peer = self.peers.resolve(source)?;
        let to_peer = self.peers.resolve(destination)?;
        let destination_monoforum_peer = destination_monoforum_peer
            .map(|peer| self.peers.resolve(peer))
            .transpose()?;
        let ids = messages
            .iter()
            .map(|message| {
                i32::try_from(message.0).map_err(|_| Error::InvalidMessageId {
                    message_id: message.0,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let random_ids = (0..messages.len())
            .map(|offset| first_random_id.wrapping_add(i64::try_from(offset).unwrap_or(i64::MAX)))
            .collect();
        self.invoke_outbound(
            &tl::functions::messages::ForwardMessages {
                silent: false,
                background: false,
                with_my_score: false,
                drop_author: false,
                drop_media_captions: false,
                from_peer,
                id: ids,
                random_id: random_ids,
                to_peer,
                top_msg_id: None,
                reply_to: destination_monoforum_peer.map(|monoforum_peer_id| {
                    tl::types::InputReplyToMonoForum { monoforum_peer_id }.into()
                }),
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
            },
            policy,
        )
        .await?;
        Ok(())
    }

    /// Sets this Account's emoji reaction on one Message.
    pub async fn react_message(
        &mut self,
        chat: ChatId,
        message: MessageId,
        reaction: String,
    ) -> Result<()> {
        self.react_message_with_policy(chat, message, reaction, InvocationPolicy::WaitForFlood)
            .await
    }

    /// Sets one Message reaction using the requested invocation policy.
    pub async fn react_message_with_policy(
        &mut self,
        chat: ChatId,
        message: MessageId,
        reaction: String,
        policy: InvocationPolicy,
    ) -> Result<()> {
        let peer = self.peers.resolve(chat)?;
        let msg_id = i32::try_from(message.0).map_err(|_| Error::InvalidMessageId {
            message_id: message.0,
        })?;
        self.invoke_outbound(
            &tl::functions::messages::SendReaction {
                big: false,
                add_to_recent: true,
                peer,
                msg_id,
                reaction: Some(vec![tl::types::ReactionEmoji { emoticon: reaction }.into()]),
            },
            policy,
        )
        .await?;
        Ok(())
    }

    /// Pins or unpins one Message where the Account has permission.
    pub async fn set_message_pinned(
        &mut self,
        chat: ChatId,
        message: MessageId,
        pinned: bool,
    ) -> Result<LiveEvent> {
        self.set_message_pinned_with_policy(chat, message, pinned, InvocationPolicy::WaitForFlood)
            .await
    }

    /// Pins or unpins one Message using the requested invocation policy.
    pub async fn set_message_pinned_with_policy(
        &mut self,
        chat: ChatId,
        message: MessageId,
        pinned: bool,
        policy: InvocationPolicy,
    ) -> Result<LiveEvent> {
        let peer = self.peers.resolve(chat)?;
        let id = i32::try_from(message.0).map_err(|_| Error::InvalidMessageId {
            message_id: message.0,
        })?;
        let response = self
            .invoke_outbound(
                &tl::functions::messages::UpdatePinnedMessage {
                    silent: false,
                    unpin: !pinned,
                    pm_oneside: false,
                    peer,
                    id,
                },
                policy,
            )
            .await?;
        let normalized = normalize_live_update(&response.to_bytes(), &mut self.names)?;
        self.peers.merge(normalized.peers.clone());
        Ok(LiveEvent {
            events: normalized.events,
            cursors: normalized.cursors,
            peers: normalized.peers,
        })
    }
}
use super::*;
