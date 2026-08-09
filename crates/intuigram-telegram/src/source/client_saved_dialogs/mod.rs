use std::collections::{HashMap, HashSet};

use super::*;

mod page;

use page::{
    SavedDialogOffset, SavedDialogPage, message_id, normalize_saved_dialog, saved_dialog_offset,
};

const SAVED_DIALOG_PAGE_SIZE: i32 = 100;

impl Client {
    /// Loads the complete ordered per-origin dialog list for Saved Messages.
    pub async fn saved_dialogs(&mut self) -> Result<Vec<SavedDialogView>> {
        match self.saved_dialogs_inner().await {
            Err(error) if error.requires_peer_refresh() => {
                self.refresh_peer_directory().await?;
                self.saved_dialogs_inner().await
            }
            result => result,
        }
    }

    async fn saved_dialogs_inner(&mut self) -> Result<Vec<SavedDialogView>> {
        let mut result = Vec::new();
        let mut seen = HashSet::new();
        let mut offset = SavedDialogOffset::default();
        loop {
            let response = self
                .connection
                .invoke(&tl::functions::messages::GetSavedDialogs {
                    exclude_pinned: !result.is_empty(),
                    parent_peer: None,
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
    pub async fn saved_history(&mut self, peer: ChatId, limit: i32) -> Result<Vec<MessageView>> {
        match self.saved_history_inner(peer, limit).await {
            Err(error) if error.requires_peer_refresh() => {
                self.refresh_peer_directory().await?;
                self.saved_history_inner(peer, limit).await
            }
            result => result,
        }
    }

    async fn saved_history_inner(&mut self, peer: ChatId, limit: i32) -> Result<Vec<MessageView>> {
        let peer_input = self.peers.resolve(peer)?;
        let response = self
            .connection
            .invoke(&tl::functions::messages::GetSavedHistory {
                parent_peer: None,
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
}
