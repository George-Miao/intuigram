//! Synchronous execution of application effects against scripted adapters.

use intuigram::encode_stored_message;
use intuigram_app::{
    AdapterEvent, AttachmentId, AttachmentKind, AttachmentView, ConnectionState, Effect,
    MediaPreviewView,
};
use intuigram_store::{StoredDraft, StoredSelection};
use intuigram_telegram::{LiveEvent, UpdateCursor, UpdateScope};
use snafu::ResultExt;

use super::TestSystem;
use super::downloads::ONE_PIXEL_PNG;
use super::telegram_control::block_on;
use crate::error::{Error, Result, StoreSnafu};
use crate::telegram::{ObservedSend, ScenarioMismatch};

impl TestSystem {
    pub(super) fn drain_effects(&mut self) -> Result<()> {
        while let Some(effect) = self.application.take_effect() {
            self.trace.borrow_mut().record(
                "effect",
                format!("{effect:?}"),
                self.application.revision(),
            );
            match effect {
                Effect::LoadScheduledMessages { chat } => self.handle_scheduled_load(chat),
                Effect::ScheduledOperation { chat, request } => {
                    self.handle_scheduled_operation(chat, request);
                }
                Effect::BrowseRichMedia { kind } => self.handle_rich_media_browse(kind),
                Effect::SendLibraryMedia { chat, local_id, .. }
                | Effect::SendRichMediaFile { chat, local_id, .. }
                | Effect::RecordRichMedia { chat, local_id, .. }
                | Effect::SendContact { chat, local_id, .. } => {
                    self.handle_rich_media_ack(chat, local_id);
                }
                Effect::FolderOperation { operation } => {
                    self.handle_folder_operation(operation);
                }
                Effect::RefreshFolders => self.handle_folder_refresh(),
                Effect::AccountLifecycle { request } => {
                    self.account_lifecycle.push(request);
                }
                Effect::Notify { chat, .. } => self.notifications.push(chat),
                Effect::LoadChat {
                    chat,
                    selection,
                    transcript_anchors,
                } => self.handle_history_load(chat, selection, transcript_anchors)?,
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
                Effect::ReadHistory { chat, max_id } => {
                    let acknowledge = self
                        .telegram
                        .read_history(chat, max_id)
                        .map_err(|error| self.scenario_error(error))?;
                    if acknowledge {
                        self.application.handle_adapter(AdapterEvent::HistoryRead {
                            chat,
                            max_id,
                            outgoing: false,
                            unread: Some(0),
                        });
                    }
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
                Effect::SelectAttachment {
                    chat,
                    thread_root,
                    path,
                } => {
                    self.next_attachment_id = self.next_attachment_id.saturating_add(1);
                    let id = AttachmentId(self.next_attachment_id);
                    let name = std::path::Path::new(&path).file_name().map_or_else(
                        || "attachment".to_owned(),
                        |name| name.to_string_lossy().into_owned(),
                    );
                    let kind = if name.ends_with(".png") || name.ends_with(".jpg") {
                        AttachmentKind::Photo
                    } else {
                        AttachmentKind::File
                    };
                    self.attachment_names.insert(id, name.clone());
                    self.application
                        .handle_adapter(AdapterEvent::ClipboardReady {
                            chat,
                            thread_root,
                            text: None,
                            attachments: vec![AttachmentView { id, kind, name }],
                        });
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
                    attachments,
                    draft_attachments: _,
                } => {
                    let attachments = attachments
                        .iter()
                        .filter_map(|id| self.attachment_names.get(id).cloned())
                        .collect();
                    let updated = self
                        .telegram
                        .edit_message(
                            chat,
                            message.id,
                            message.body.clone(),
                            message.details.entities.clone(),
                            attachments,
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
                Effect::LoadAvatar { avatar } => {
                    self.handle_avatar_load(avatar)?;
                }
                Effect::OpenDownload { download, reveal } => {
                    self.opened_downloads.push((download, reveal));
                    self.application
                        .handle_adapter(AdapterEvent::OperationCompleted(
                            "Opened completed download".to_owned(),
                        ));
                }
                Effect::PickAttachment { chat, thread_root } => {
                    self.application
                        .handle_adapter(AdapterEvent::AttachmentPathRequired { chat, thread_root });
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

    pub(super) fn scenario_error(&self, error: ScenarioMismatch) -> Error {
        Error::TelegramMismatch {
            expected: error.expected,
            observed: error.observed,
            artifact: self.trace.borrow().persist(),
        }
    }
}
