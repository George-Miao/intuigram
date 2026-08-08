impl Client {
    /// Loads one bounded recent-history page for a cached Chat.
    pub async fn history(&mut self, chat: ChatId, limit: i32) -> Result<Vec<MessageView>> {
        match self.history_page(chat, 0, limit).await {
            Err(error) if error.requires_peer_refresh() => {
                self.refresh_peer_directory().await?;
                self.history_page(chat, 0, limit).await
            }
            result => result,
        }
    }

    async fn history_page(
        &mut self,
        chat: ChatId,
        offset_id: i32,
        limit: i32,
    ) -> Result<Vec<MessageView>> {
        let peer = self.peers.resolve(chat)?;
        let response = self
            .connection
            .invoke(&tl::functions::messages::GetHistory {
                peer,
                offset_id,
                offset_date: 0,
                add_offset: 0,
                limit,
                max_id: 0,
                min_id: 0,
                hash: 0,
            })
            .await
            .context(InvokeSnafu)?;
        let (mut messages, chats, users) = message_parts(response);
        self.update_peer_cache(&chats, &users);
        messages.reverse();
        Ok(messages
            .iter()
            .filter_map(|message| normalize_message(message, &self.names))
            .collect())
    }

    /// Loads one bounded pinned-Message projection for a cached Chat.
    pub async fn pinned_messages(&mut self, chat: ChatId, limit: i32) -> Result<Vec<MessageView>> {
        let peer = self.peers.resolve(chat)?;
        let response = self
            .connection
            .invoke(&tl::functions::messages::Search {
                peer,
                q: String::new(),
                from_id: None,
                saved_peer_id: None,
                saved_reaction: None,
                top_msg_id: None,
                filter: tl::enums::MessagesFilter::InputMessagesFilterPinned,
                min_date: 0,
                max_date: 0,
                offset_id: 0,
                add_offset: 0,
                limit,
                max_id: 0,
                min_id: 0,
                hash: 0,
            })
            .await
            .context(InvokeSnafu)?;
        let (mut messages, chats, users) = message_parts(response);
        self.update_peer_cache(&chats, &users);
        messages.reverse();
        Ok(messages
            .iter()
            .filter_map(|message| normalize_message(message, &self.names))
            .collect())
    }

    /// Loads an ordinary Message Thread or Channel comment history.
    pub async fn thread_history(
        &mut self,
        chat: ChatId,
        root: MessageId,
        limit: i32,
    ) -> Result<Vec<MessageView>> {
        let peer = self.peers.resolve(chat)?;
        let root =
            i32::try_from(root.0).map_err(|_| Error::InvalidMessageId { message_id: root.0 })?;
        let response = self
            .connection
            .invoke(&tl::functions::messages::GetReplies {
                peer,
                msg_id: root,
                offset_id: 0,
                offset_date: 0,
                add_offset: 0,
                limit,
                max_id: 0,
                min_id: 0,
                hash: 0,
            })
            .await
            .context(InvokeSnafu)?;
        let (mut messages, chats, users) = message_parts(response);
        self.update_peer_cache(&chats, &users);
        messages.reverse();
        Ok(messages
            .iter()
            .filter_map(|message| normalize_message(message, &self.names))
            .collect())
    }

    /// Acknowledges incoming Messages visible in one ordinary Thread or
    /// Channel comment history.
    pub async fn read_thread(
        &mut self,
        chat: ChatId,
        root: MessageId,
        max_id: MessageId,
    ) -> Result<()> {
        let peer = self.peers.resolve(chat)?;
        let request = read_request(peer, root, max_id)?;
        self.connection
            .invoke(&request)
            .await
            .context(InvokeSnafu)?;
        Ok(())
    }

    /// Acknowledges incoming Messages visible in root Chat history.
    pub async fn read_history(&mut self, chat: ChatId, max_id: MessageId) -> Result<()> {
        let peer = self.peers.resolve(chat)?;
        let max_id = i32::try_from(max_id.0).map_err(|_| Error::InvalidMessageId {
            message_id: max_id.0,
        })?;
        if let tl::enums::InputPeer::Channel(channel) = peer {
            self.connection
                .invoke(&tl::functions::channels::ReadHistory {
                    channel: tl::types::InputChannel {
                        channel_id: channel.channel_id,
                        access_hash: channel.access_hash,
                    }
                    .into(),
                    max_id,
                })
                .await
                .context(InvokeSnafu)?;
        } else {
            self.connection
                .invoke(&tl::functions::messages::ReadHistory { peer, max_id })
                .await
                .context(InvokeSnafu)?;
        }
        Ok(())
    }
}

use super::*;
