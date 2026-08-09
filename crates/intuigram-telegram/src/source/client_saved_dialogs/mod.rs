use std::collections::{HashMap, HashSet};

use super::*;

mod page;

use page::{
    SavedDialogOffset, SavedDialogPage, message_id, normalize_saved_dialog, saved_dialog_offset,
};

const SAVED_DIALOG_PAGE_SIZE: i32 = 100;

impl Client {
    /// Loads the complete ordered per-origin dialog list for Saved Messages.
    pub async fn saved_dialogs(&mut self, chat: ChatId) -> Result<Vec<SavedDialogView>> {
        match self.saved_dialogs_inner(chat).await {
            Err(error) if error.requires_peer_refresh() => {
                self.refresh_peer_directory().await?;
                self.saved_dialogs_inner(chat).await
            }
            result => result,
        }
    }

    async fn saved_dialogs_inner(&mut self, chat: ChatId) -> Result<Vec<SavedDialogView>> {
        let parent_peer = self.saved_parent_peer(chat)?;
        let mut result = Vec::new();
        let mut seen = HashSet::new();
        let mut offset = SavedDialogOffset::default();
        loop {
            let response = self
                .connection
                .invoke(&tl::functions::messages::GetSavedDialogs {
                    exclude_pinned: parent_peer.is_none() && !result.is_empty(),
                    parent_peer: parent_peer.clone(),
                    offset_date: offset.date,
                    offset_id: offset.message,
                    offset_peer: offset.peer.clone(),
                    limit: SAVED_DIALOG_PAGE_SIZE,
                    hash: 0,
                })
                .await
                .context(InvokeSnafu)?;
            let page = SavedDialogPage::from_response(response);
            self.update_peer_cache(&page.chats, &page.users);
            let messages = page
                .messages
                .iter()
                .map(|message| (message_id(message), message))
                .collect::<HashMap<_, _>>();
            let next = page
                .dialogs
                .last()
                .map(|dialog| saved_dialog_offset(dialog, &messages, &self.peers))
                .transpose()?;
            let page_len = page.dialogs.len();
            for dialog in page.dialogs {
                let peer = marked_peer_id(&dialog.peer());
                let top_message = dialog.top_message();
                if seen.insert(peer) {
                    result.push(normalize_saved_dialog(
                        dialog,
                        messages.get(&top_message),
                        &self.names,
                    ));
                }
            }
            let complete = page.total.is_none_or(|count| result.len() >= count)
                || page_len < usize::try_from(SAVED_DIALOG_PAGE_SIZE).unwrap_or(100);
            let Some(next) = next else {
                break;
            };
            if complete || next == offset {
                break;
            }
            offset = next;
        }
        Ok(result)
    }

    /// Loads one bounded history page filtered to an origin in Saved Messages.
    pub async fn saved_history(
        &mut self,
        chat: ChatId,
        peer: ChatId,
        limit: i32,
    ) -> Result<Vec<MessageView>> {
        match self.saved_history_inner(chat, peer, limit).await {
            Err(error) if error.requires_peer_refresh() => {
                self.refresh_peer_directory().await?;
                self.saved_history_inner(chat, peer, limit).await
            }
            result => result,
        }
    }

    async fn saved_history_inner(
        &mut self,
        chat: ChatId,
        peer: ChatId,
        limit: i32,
    ) -> Result<Vec<MessageView>> {
        let parent_peer = self.saved_parent_peer(chat)?;
        let peer_input = self.peers.resolve(peer)?;
        let response = self
            .connection
            .invoke(&tl::functions::messages::GetSavedHistory {
                parent_peer,
                peer: peer_input,
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
            .map(|mut message| {
                message.details.saved_peer = Some(peer);
                message
            })
            .collect())
    }

    /// Acknowledges incoming Messages visible in one administrator-owned
    /// monoforum user dialog.
    pub async fn read_saved_history(
        &mut self,
        chat: ChatId,
        peer: ChatId,
        max_id: MessageId,
    ) -> Result<()> {
        let parent_peer = self.peers.resolve(chat)?;
        let peer = self.peers.resolve(peer)?;
        let max_id = i32::try_from(max_id.0).map_err(|_| Error::InvalidMessageId {
            message_id: max_id.0,
        })?;
        self.connection
            .invoke(&tl::functions::messages::ReadSavedHistory {
                parent_peer,
                peer,
                max_id,
            })
            .await
            .context(InvokeSnafu)?;
        Ok(())
    }

    fn saved_parent_peer(&self, chat: ChatId) -> Result<Option<tl::enums::InputPeer>> {
        if self
            .identity
            .as_ref()
            .is_some_and(|identity| identity.id == chat.0)
        {
            Ok(None)
        } else {
            self.peers.resolve(chat).map(Some)
        }
    }
}
