//! Chat-history effect execution through the real durable adapter seam.

use intuigram_app::encode_stored_message;
use intuigram_lib::{AdapterEvent, ChatId, SelectionView, TranscriptAnchorView};
use intuigram_store::{StoredSelection, StoredTranscriptAnchor};
use snafu::ResultExt;

use super::TestSystem;
use super::telegram_control::block_on;
use crate::error::{Result, StoreSnafu};
use crate::telegram::HistoryResult;

impl TestSystem {
    pub(super) fn handle_history_load(
        &mut self,
        chat: ChatId,
        selection: Option<SelectionView>,
        transcript_anchors: Vec<TranscriptAnchorView>,
    ) -> Result<()> {
        if let Some(selection) = selection {
            self.database
                .save_selection(StoredSelection {
                    folder_id: selection.folder,
                    chat_id: selection.chat.map(|chat| chat.0),
                    anchor_message_id: selection.message.map(|message| message.0),
                    transcript_anchors: transcript_anchors
                        .into_iter()
                        .map(|anchor| StoredTranscriptAnchor {
                            chat_id: anchor.chat.0,
                            thread_root: anchor.thread.map(|message| message.0),
                            saved_peer: anchor.saved_peer.map(|peer| peer.0),
                            message_id: anchor.message.0,
                        })
                        .collect(),
                })
                .context(StoreSnafu)?;
        }
        let result = self
            .telegram
            .load_history(chat)
            .map_err(|error| self.scenario_error(error))?;
        if let HistoryResult::Loaded {
            status,
            messages,
            pinned_messages,
        } = &result
        {
            let request = self
                .database
                .store()
                .save_chat_history(
                    chat.0,
                    messages
                        .iter()
                        .map(|message| encode_stored_message(chat, message))
                        .collect(),
                    pinned_messages
                        .iter()
                        .map(|message| encode_stored_message(chat, message))
                        .collect(),
                    status.clone(),
                )
                .context(StoreSnafu)?;
            block_on(request).context(StoreSnafu)?;
        }
        self.application.handle_adapter(match result {
            HistoryResult::Loaded {
                status,
                messages,
                pinned_messages,
            } => AdapterEvent::ChatLoaded {
                chat,
                avatar_peers: Vec::new(),
                status,
                messages,
                pinned_messages,
            },
            HistoryResult::Failed(reason) => AdapterEvent::HistoryLoadFailed {
                chat,
                thread_root: None,
                saved_peer: None,
                reason,
            },
        });
        Ok(())
    }
}
