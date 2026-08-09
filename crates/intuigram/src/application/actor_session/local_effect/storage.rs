use std::cell::RefCell;

use compio::runtime::ResumeUnwind;
use intuigram_app::{AdapterEvent, Effect, OfflineMediaFailure};
use intuigram_store::{AccountStore, StoredDraft, StoredSelection, StoredTranscriptAnchor};
use snafu::ResultExt;

use super::super::super::{AccountDatabaseSnafu, MediaCacheSnafu, Result, unix_timestamp};
use super::State;

pub(super) async fn execute(
    effect: Effect,
    store: &AccountStore,
    state: &RefCell<State>,
) -> Result<Option<AdapterEvent>> {
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
        Effect::SetChatMediaOffline(policy) => {
            let persisted = match store.set_chat_media_offline(policy.chat.0, policy.keep) {
                Ok(request) => request.await.context(AccountDatabaseSnafu),
                Err(error) => Err(error).context(AccountDatabaseSnafu),
            };
            if let Err(error) = persisted {
                return Ok(Some(AdapterEvent::ChatMediaOfflineFailed(
                    OfflineMediaFailure {
                        chat: policy.chat,
                        message: None,
                        reason: error.to_string(),
                    },
                )));
            }
            if !policy.keep {
                let cache = state.borrow().media_cache.clone();
                let owner = media_owner(policy.chat);
                let released = compio::runtime::spawn_blocking(move || cache.release(&owner))
                    .await
                    .resume_unwind()
                    .expect("an awaited retained-media release cannot be cancelled")
                    .context(MediaCacheSnafu);
                if let Err(error) = released {
                    if let Ok(request) = store.set_chat_media_offline(policy.chat.0, true) {
                        let _ = request.await;
                    }
                    return Ok(Some(AdapterEvent::ChatMediaOfflineFailed(
                        OfflineMediaFailure {
                            chat: policy.chat,
                            message: None,
                            reason: error.to_string(),
                        },
                    )));
                }
            }
            return Ok(Some(AdapterEvent::ChatMediaOfflineChanged(policy)));
        }
        _ => unreachable!("only ordered local effects reach local storage"),
    }
    Ok(None)
}

fn media_owner(chat: intuigram_app::ChatId) -> intuigram_media::CacheOwner {
    intuigram_media::CacheOwner::new(format!("chat:{}", chat.0))
}
