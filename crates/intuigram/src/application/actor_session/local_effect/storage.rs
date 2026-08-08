use intuigram_app::Effect;
use intuigram_store::{AccountStore, StoredDraft, StoredSelection, StoredTranscriptAnchor};
use snafu::ResultExt;

use super::super::super::{AccountDatabaseSnafu, Result, unix_timestamp};

pub(super) async fn execute(
    effect: Effect,
    store: &AccountStore,
) -> Result<Option<intuigram_app::AdapterEvent>> {
    match effect {
        Effect::SaveDraft {
            chat,
            thread_root,
            text,
            reply_to,
        } => {
            store
                .save_draft(StoredDraft {
                    chat_id: chat.0,
                    thread_root: thread_root.map(|message| message.0),
                    text,
                    reply_to: reply_to.map(|message| message.0),
                    modified_at: unix_timestamp(),
                })
                .context(AccountDatabaseSnafu)?
                .await
                .context(AccountDatabaseSnafu)?;
        }
        Effect::SaveSelection {
            folder,
            chat,
            message,
            transcript_anchors,
        } => {
            store
                .save_selection(StoredSelection {
                    folder_id: folder,
                    chat_id: chat.map(|chat| chat.0),
                    anchor_message_id: message.map(|message| message.0),
                    transcript_anchors: transcript_anchors
                        .into_iter()
                        .map(|anchor| StoredTranscriptAnchor {
                            chat_id: anchor.chat.0,
                            thread_root: anchor.thread.map(|message| message.0),
                            message_id: anchor.message.0,
                        })
                        .collect(),
                })
                .context(AccountDatabaseSnafu)?
                .await
                .context(AccountDatabaseSnafu)?;
        }
        _ => unreachable!("only ordered local effects reach local storage"),
    }
    Ok(None)
}
