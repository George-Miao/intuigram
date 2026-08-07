//! Synchronous execution of application effects against scripted adapters.

use intuigram::encode_stored_message;
use intuigram_app::{AdapterEvent, ConnectionState, Effect, MediaPreviewView};
use intuigram_store::{StoredDraft, StoredSelection};
use intuigram_telegram::{LiveEvent, UpdateCursor, UpdateScope};
use snafu::ResultExt;

use super::TestSystem;
use super::telegram_control::block_on;
use crate::error::{Error, Result, StoreSnafu};
use crate::telegram::{HistoryResult, ObservedSend, ScenarioMismatch};

pub(super) const ONE_PIXEL_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x04, 0x00, 0x00, 0x00, 0xb5, 0x1c, 0x0c,
    0x02, 0x00, 0x00, 0x00, 0x0b, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x64, 0xf8, 0x0f, 0x00,
    0x01, 0x05, 0x01, 0x01, 0x27, 0x18, 0xe3, 0x66, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44,
    0xae, 0x42, 0x60, 0x82,
];

impl TestSystem {
    pub(super) fn drain_effects(&mut self) -> Result<()> {
        while let Some(effect) = self.application.take_effect() {
            self.trace.borrow_mut().record(
                "effect",
                format!("{effect:?}"),
                self.application.revision(),
            );
            match effect {
                Effect::AccountLifecycle { request } => {
                    self.account_lifecycle.push(request);
                }
                Effect::Notify { .. } => {}
                Effect::LoadChat {
                    chat,
                    selection,
                    transcript_anchors,
                } => {
                    if let Some(selection) = selection {
                        self.database
                            .save_selection(StoredSelection {
                                folder_id: selection.folder,
                                chat_id: selection.chat.map(|chat| chat.0),
                                anchor_message_id: selection.message.map(|message| message.0),
                                transcript_anchors: transcript_anchors
                                    .into_iter()
                                    .map(|anchor| intuigram_store::StoredTranscriptAnchor {
                                        chat_id: anchor.chat.0,
                                        thread_root: anchor.thread.map(|message| message.0),
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
                        messages,
                        pinned_messages,
                    } = &result
                    {
                        let request = self
                            .database
                            .store()
                            .save_messages(
                                messages
                                    .iter()
                                    .chain(pinned_messages)
                                    .map(|message| encode_stored_message(chat, message))
                                    .collect(),
                            )
                            .context(StoreSnafu)?;
                        block_on(request).context(StoreSnafu)?;
                    }
                    self.application.handle_adapter(match result {
                        HistoryResult::Loaded {
                            messages,
                            pinned_messages,
                        } => AdapterEvent::ChatLoaded {
                            chat,
                            messages,
                            pinned_messages,
                        },
                        HistoryResult::Failed(reason) => AdapterEvent::HistoryLoadFailed {
                            chat,
                            thread_root: None,
                            reason,
                        },
                    });
                }
                Effect::SaveSelection {
                    folder,
                    chat,
                    message,
                    transcript_anchors,
                } => {
                    self.database
                        .save_selection(StoredSelection {
                            folder_id: folder,
                            chat_id: chat.map(|chat| chat.0),
                            anchor_message_id: message.map(|message| message.0),
                            transcript_anchors: transcript_anchors
                                .into_iter()
                                .map(|anchor| intuigram_store::StoredTranscriptAnchor {
                                    chat_id: anchor.chat.0,
                                    thread_root: anchor.thread.map(|message| message.0),
                                    message_id: anchor.message.0,
                                })
                                .collect(),
                        })
                        .context(StoreSnafu)?;
                }
                Effect::LoadThread { chat, root } => {
                    let messages = self
                        .telegram
                        .load_thread(chat, root)
                        .map_err(|error| self.scenario_error(error))?;
                    self.application.handle_adapter(AdapterEvent::ThreadLoaded {
                        chat,
                        root,
                        messages,
                    });
                }
                Effect::ReadThread { chat, root, max_id } => {
                    self.telegram
                        .read_thread(chat, root, max_id)
                        .map_err(|error| self.scenario_error(error))?;
                }
                Effect::SaveDraft {
                    chat,
                    thread_root,
                    text,
                    reply_to,
                } => {
                    self.database
                        .save_draft(StoredDraft {
                            chat_id: chat.0,
                            thread_root: thread_root.map(|message| message.0),
                            text,
                            reply_to: reply_to.map(|message| message.0),
                            modified_at: 0,
                        })
                        .context(StoreSnafu)?;
                }
                Effect::SendMessage {
                    chat,
                    text,
                    entities,
                    link_preview,
                    reply_to,
                    thread_root,
                    attachments,
                    local_id,
                } if attachments.is_empty() => {
                    self.database
                        .save_draft(StoredDraft {
                            chat_id: chat.0,
                            thread_root: thread_root.map(|message| message.0),
                            text: String::new(),
                            reply_to: None,
                            modified_at: 0,
                        })
                        .context(StoreSnafu)?;
                    self.telegram
                        .hold_send(ObservedSend {
                            chat,
                            text,
                            entities,
                            link_preview,
                            reply_to,
                            thread_root,
                            local_id,
                        })
                        .map_err(|error| self.scenario_error(error))?;
                }
                Effect::SendPoll {
                    chat,
                    question,
                    options,
                    reply_to,
                    thread_root,
                    local_id,
                } => {
                    self.telegram
                        .send_poll(chat, question, options, reply_to, thread_root)
                        .map_err(|error| self.scenario_error(error))?;
                    self.application
                        .handle_adapter(AdapterEvent::MessageAcknowledged { chat, local_id });
                }
                Effect::EditMessage {
                    chat,
                    message,
                    draft_text: _,
                } => {
                    let updated = self
                        .telegram
                        .edit_message(
                            chat,
                            message.id,
                            message.body.clone(),
                            message.details.entities.clone(),
                        )
                        .map_err(|error| self.scenario_error(error))?;
                    let request = self
                        .database
                        .store()
                        .save_messages(vec![encode_stored_message(chat, &updated)])
                        .context(StoreSnafu)?;
                    block_on(request).context(StoreSnafu)?;
                    self.application
                        .handle_adapter(AdapterEvent::MessageUpdated {
                            chat,
                            message: Box::new(updated),
                        });
                }
                Effect::DeleteMessages { chat, messages } => {
                    self.telegram
                        .delete_messages(chat, messages.clone())
                        .map_err(|error| self.scenario_error(error))?;
                    let request = self
                        .database
                        .store()
                        .delete_messages(
                            Some(chat.0),
                            messages.iter().map(|message| message.0).collect(),
                        )
                        .context(StoreSnafu)?;
                    block_on(request).context(StoreSnafu)?;
                    self.application
                        .handle_adapter(AdapterEvent::MessagesDeleted {
                            chat: Some(chat),
                            ids: messages,
                        });
                }
                Effect::ForwardMessages {
                    source,
                    destination,
                    messages,
                } => {
                    self.telegram
                        .forward_messages(source, destination, messages)
                        .map_err(|error| self.scenario_error(error))?;
                }
                Effect::ReactMessage {
                    chat,
                    message,
                    reaction,
                } => {
                    let updated = self
                        .telegram
                        .react_message(chat, message.id, reaction)
                        .map_err(|error| self.scenario_error(error))?;
                    let request = self
                        .database
                        .store()
                        .save_messages(vec![encode_stored_message(chat, &updated)])
                        .context(StoreSnafu)?;
                    block_on(request).context(StoreSnafu)?;
                    self.application
                        .handle_adapter(AdapterEvent::MessageUpdated {
                            chat,
                            message: Box::new(updated),
                        });
                }
                Effect::SetMessagePinned {
                    chat,
                    message,
                    pinned,
                } => {
                    let updated = self
                        .telegram
                        .set_message_pinned(chat, message, pinned)
                        .map_err(|error| self.scenario_error(error))?;
                    if updated.details.pinned != pinned {
                        return Err(Error::Expectation {
                            expectation: format!("Telegram pin result is {pinned}"),
                            actual: format!("{}", updated.details.pinned),
                            artifact: self.trace.borrow().persist(),
                        });
                    }
                    self.next_update_pts = self.next_update_pts.saturating_add(1);
                    let commit = self
                        .updates
                        .commit(LiveEvent {
                            events: vec![AdapterEvent::MessagesPinChanged {
                                chat,
                                ids: vec![message],
                                pinned,
                            }],
                            cursors: vec![UpdateCursor {
                                scope: UpdateScope::Account,
                                pts: Some(self.next_update_pts),
                                pts_count: 1,
                                ..UpdateCursor::default()
                            }],
                            peers: intuigram_telegram::PeerDirectory::default(),
                        })
                        .context(crate::error::SyncSnafu)?;
                    let committed = block_on(commit).context(crate::error::SyncSnafu)?;
                    for event in committed.events {
                        self.application.handle_adapter(event);
                    }
                }
                Effect::VotePoll {
                    chat,
                    message,
                    options,
                } => {
                    let updated = self
                        .telegram
                        .vote_poll(chat, message.id, options)
                        .map_err(|error| self.scenario_error(error))?;
                    let request = self
                        .database
                        .store()
                        .save_messages(vec![encode_stored_message(chat, &updated)])
                        .context(StoreSnafu)?;
                    block_on(request).context(StoreSnafu)?;
                    self.application
                        .handle_adapter(AdapterEvent::MessageUpdated {
                            chat,
                            message: Box::new(updated),
                        });
                }
                Effect::OpenExternalLink { url } => {
                    self.opened_links.push(url.clone());
                    self.application
                        .handle_adapter(AdapterEvent::OperationCompleted(format!("Opened {url}")));
                }
                Effect::DownloadMedia {
                    chat,
                    message,
                    destination,
                } => {
                    self.download_media_effect(chat, message, destination)?;
                }
                Effect::LoadMediaPreview { chat, message } => {
                    self.telegram
                        .load_media_preview(chat, message)
                        .map_err(|error| self.scenario_error(error))?;
                    let image = intuigram_media::decode_preview(ONE_PIXEL_PNG)
                        .expect("the committed behavior PNG should decode");
                    self.application
                        .handle_adapter(AdapterEvent::MediaPreviewReady(MediaPreviewView {
                            chat,
                            message,
                            image,
                        }));
                }
                Effect::OpenDownload { download, reveal } => {
                    self.opened_downloads.push((download, reveal));
                    self.application
                        .handle_adapter(AdapterEvent::OperationCompleted(
                            "Opened completed download".to_owned(),
                        ));
                }
                Effect::Reconnect => {
                    self.telegram
                        .reconnect()
                        .map_err(|error| self.scenario_error(error))?;
                    self.application
                        .handle_adapter(AdapterEvent::ConnectionChanged(
                            ConnectionState::Connected,
                        ));
                }
                Effect::Quit => {}
                effect => {
                    return Err(Error::UnexpectedEffect {
                        effect: format!("{effect:?}"),
                        artifact: self.trace.borrow().persist(),
                    });
                }
            }
            self.render();
        }
        Ok(())
    }

    fn scenario_error(&self, error: ScenarioMismatch) -> Error {
        Error::TelegramMismatch {
            expected: error.expected,
            observed: error.observed,
            artifact: self.trace.borrow().persist(),
        }
    }
}
