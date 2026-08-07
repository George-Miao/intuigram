impl Client {
    /// Loads one bounded recent-history page for a cached Chat.
    pub async fn history(&mut self, chat: ChatId, limit: i32) -> Result<Vec<MessageView>> {
        Ok(self.history_page(chat, 0, limit).await?.messages)
    }

    /// Loads a Chat's complete Message History through bounded Telegram pages.
    pub async fn complete_history(
        &mut self,
        chat: ChatId,
        page_size: i32,
    ) -> Result<Vec<MessageView>> {
        match self.complete_history_once(chat, page_size).await {
            Err(error) if error.requires_peer_refresh() => {
                self.refresh_peer_directory().await?;
                self.complete_history_once(chat, page_size).await
            }
            result => result,
        }
    }

    async fn complete_history_once(
        &mut self,
        chat: ChatId,
        page_size: i32,
    ) -> Result<Vec<MessageView>> {
        let page_size = page_size.clamp(1, 100);
        let mut offset = 0;
        let mut pages = Vec::new();
        loop {
            let page = self.history_page(chat, offset, page_size).await?;
            let complete = page.next_offset.is_none();
            offset = page.next_offset.unwrap_or_default();
            pages.push(page.messages);
            if complete {
                break;
            }
        }
        pages.reverse();
        Ok(pages.into_iter().flatten().collect())
    }

    async fn history_page(
        &mut self,
        chat: ChatId,
        offset_id: i32,
        limit: i32,
    ) -> Result<HistoryPage> {
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
        let count = messages.len();
        let next_offset = if count < limit as usize {
            None
        } else {
            messages
                .iter()
                .map(tl::enums::Message::id)
                .min()
                .filter(|next| *next != offset_id)
        };
        if count == limit as usize && next_offset.is_none() {
            return HistoryOffsetUnavailableSnafu { chat_id: chat.0 }.fail();
        }
        self.update_peer_cache(&chats, &users);
        messages.reverse();
        Ok(HistoryPage {
            messages: messages
                .iter()
                .filter_map(|message| normalize_message(message, &self.names))
                .collect(),
            next_offset,
        })
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
}

struct HistoryPage {
    messages: Vec<MessageView>,
    next_offset: Option<i32>,
}
use super::*;
