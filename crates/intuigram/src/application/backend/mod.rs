use super::*;

mod attachments;
mod download;
mod effects;
mod folders;
mod history_failure;
mod location;
mod message_actions;
mod pins;
mod poll;
mod reads;
mod rich_media;
mod saved_dialogs;
mod scheduled;
mod send;
mod specialized;
mod topics;

use history_failure::history_failure_event;
use poll::PollPersistence;
pub(super) use rich_media::upload_kind;

pub(super) struct MessageSend {
    pub(super) chat: ChatId,
    pub(super) text: String,
    pub(super) entities: Vec<TextEntity>,
    pub(super) link_preview: bool,
    pub(super) reply_to: Option<MessageId>,
    pub(super) thread_root: Option<MessageId>,
    pub(super) saved_peer: Option<ChatId>,
    pub(super) attachment_ids: Vec<AttachmentId>,
    pub(super) random_id: i64,
}

pub(super) struct OutgoingRecord<'a> {
    pub(super) chat: ChatId,
    pub(super) local_id: MessageId,
    pub(super) text: &'a str,
    pub(super) entities: &'a [TextEntity],
    pub(super) reply_to: Option<MessageId>,
    pub(super) thread_root: Option<MessageId>,
    pub(super) saved_peer: Option<ChatId>,
    pub(super) delivery: DeliveryState,
}

impl Backend {
    fn attachment_store(&mut self) -> &mut AttachmentStore {
        &mut self.attachments
    }

    pub(super) async fn read_clipboard(
        &mut self,
        chat: ChatId,
        thread_root: Option<MessageId>,
        saved_peer: Option<ChatId>,
    ) -> Result<AdapterEvent> {
        let content = rich_clipboard::read().await.context(ClipboardSnafu)?;
        let (text, attachments) = match content {
            rich_clipboard::ClipboardContent::Text(text) => (Some(text), Vec::new()),
            rich_clipboard::ClipboardContent::Image { mime_type, bytes } => {
                let id = self
                    .attachment_store()
                    .register(AttachmentPayload::Image { mime_type, bytes });
                (
                    None,
                    vec![AttachmentView {
                        id,
                        kind: AttachmentKind::Photo,
                        name: "clipboard.png".to_owned(),
                    }],
                )
            }
            rich_clipboard::ClipboardContent::Files(paths) => {
                let attachments = paths
                    .into_iter()
                    .map(|path| {
                        let name = path.file_name().map_or_else(
                            || "attachment".to_owned(),
                            |name| name.to_string_lossy().into_owned(),
                        );
                        let kind = if mime_type_for_path(&path).starts_with("video/") {
                            AttachmentKind::Video
                        } else {
                            AttachmentKind::File
                        };
                        let id = self
                            .attachment_store()
                            .register(AttachmentPayload::File { path, kind });
                        AttachmentView { id, kind, name }
                    })
                    .collect();
                (None, attachments)
            }
        };
        Ok(AdapterEvent::ClipboardReady {
            chat,
            thread_root,
            saved_peer,
            text,
            attachments,
        })
    }

    pub(super) async fn save_draft(
        &mut self,
        chat: ChatId,
        thread_root: Option<MessageId>,
        saved_peer: Option<ChatId>,
        text: String,
        reply_to: Option<MessageId>,
    ) -> Result<()> {
        self.store
            .save_draft(intuigram_store::StoredDraft {
                chat_id: chat.0,
                thread_root: thread_root.map(|message| message.0),
                saved_peer: saved_peer.map(|peer| peer.0),
                text,
                reply_to: reply_to.map(|message| message.0),
                modified_at: unix_timestamp(),
            })
            .context(AccountDatabaseSnafu)?
            .await
            .context(AccountDatabaseSnafu)
    }

    pub(super) async fn save_selection(
        &mut self,
        folder: i32,
        chat: Option<ChatId>,
        message: Option<MessageId>,
        transcript_anchors: Vec<TranscriptAnchorView>,
    ) -> Result<()> {
        self.store
            .save_selection(intuigram_store::StoredSelection {
                folder_id: folder,
                chat_id: chat.map(|chat| chat.0),
                anchor_message_id: message.map(|message| message.0),
                transcript_anchors: transcript_anchors
                    .into_iter()
                    .map(|anchor| intuigram_store::StoredTranscriptAnchor {
                        chat_id: anchor.chat.0,
                        thread_root: anchor.thread.map(|message| message.0),
                        saved_peer: anchor.saved_peer.map(|peer| peer.0),
                        message_id: anchor.message.0,
                    })
                    .collect(),
            })
            .context(AccountDatabaseSnafu)?
            .await
            .context(AccountDatabaseSnafu)
    }

    pub(super) async fn load_chat(
        &mut self,
        chat: ChatId,
        refresh_status: bool,
    ) -> Result<(Vec<MessageView>, Vec<MessageView>, Option<String>)> {
        let messages = self
            .client
            .history(chat, 100)
            .await
            .context(TelegramSnafu)?;
        let pinned_messages = self
            .client
            .pinned_messages(chat, 100)
            .await
            .context(TelegramSnafu)?;
        let status = if refresh_status {
            self.client.chat_status(chat).await.ok().flatten()
        } else {
            None
        };
        self.store
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
            .context(AccountDatabaseSnafu)?
            .await
            .context(AccountDatabaseSnafu)?;
        Ok((messages, pinned_messages, status))
    }

    pub(super) async fn load_selected_chat(
        &mut self,
        chat: ChatId,
        selection: Option<SelectionView>,
        transcript_anchors: Vec<TranscriptAnchorView>,
    ) -> Result<Option<AdapterEvent>> {
        let refresh_status = selection.is_some();
        if let Some(selection) = selection {
            self.save_selection(
                selection.folder,
                selection.chat,
                selection.message,
                transcript_anchors,
            )
            .await?;
        }
        match self.load_chat(chat, refresh_status).await {
            Ok((messages, pinned_messages, status)) => Ok(Some(AdapterEvent::ChatLoaded {
                chat,
                status,
                messages,
                pinned_messages,
            })),
            Err(error) => history_failure_event(chat, None, None, error),
        }
    }

    pub(super) async fn load_thread(
        &mut self,
        chat: ChatId,
        root: MessageId,
    ) -> Result<Vec<MessageView>> {
        let messages = self
            .client
            .thread_history(chat, root, 100)
            .await
            .context(TelegramSnafu)?;
        self.store
            .save_messages(
                messages
                    .iter()
                    .map(|message| encode_stored_message(chat, message))
                    .collect(),
            )
            .context(AccountDatabaseSnafu)?
            .await
            .context(AccountDatabaseSnafu)?;
        Ok(messages)
    }

    pub(super) async fn persist_outgoing(&mut self, record: OutgoingRecord<'_>) -> Result<()> {
        let message = outgoing_message(&record, record.local_id, record.delivery);
        self.store
            .save_messages(vec![encode_stored_message(record.chat, &message)])
            .context(AccountDatabaseSnafu)?
            .await
            .context(AccountDatabaseSnafu)
    }

    pub(super) async fn acknowledge_outgoing(
        &mut self,
        record: OutgoingRecord<'_>,
        server_id: MessageId,
    ) -> Result<()> {
        let message = outgoing_message(&record, server_id, DeliveryState::Sent);
        self.store
            .replace_message(
                record.chat.0,
                record.local_id.0,
                encode_stored_message(record.chat, &message),
            )
            .context(AccountDatabaseSnafu)?
            .await
            .context(AccountDatabaseSnafu)
    }

    pub(super) async fn send_message(&mut self, request: MessageSend) -> Result<MessageView> {
        let MessageSend {
            chat,
            text,
            mut entities,
            link_preview,
            reply_to,
            thread_root,
            saved_peer,
            attachment_ids,
            random_id,
        } = request;
        let message_id = {
            let Self {
                client,
                attachments,
                ..
            } = self;
            if attachment_ids.is_empty() {
                client
                    .send_text(intuigram_telegram::TextSend {
                        chat,
                        text: text.clone(),
                        entities,
                        link_preview,
                        reply_to,
                        thread_root,
                        monoforum_peer: saved_peer,
                        random_id,
                        schedule_date: None,
                    })
                    .await
                    .context(TelegramSnafu)?
            } else {
                let payloads = attachment_ids
                    .iter()
                    .map(|id| {
                        attachments
                            .payloads
                            .get(id)
                            .cloned()
                            .map(|payload| (*id, payload))
                            .ok_or(Error::MissingPreparedAttachment { attachment: *id })
                    })
                    .collect::<Result<Vec<_>>>()?;
                let mut message_id = None;
                for (index, (_, payload)) in payloads.iter().enumerate() {
                    let upload = attachments::prepared_upload(payload)?;
                    let sent = client
                        .send_upload(intuigram_telegram::UploadSend {
                            chat,
                            upload,
                            caption: if index == 0 {
                                text.clone()
                            } else {
                                String::new()
                            },
                            entities: if index == 0 {
                                std::mem::take(&mut entities)
                            } else {
                                Vec::new()
                            },
                            reply_to,
                            thread_root,
                            monoforum_peer: saved_peer,
                            ids: intuigram_telegram::UploadIds {
                                file: derived_random_id(random_id, index, 0x4649_4c45),
                                message: derived_random_id(random_id, index, 0x4d45_5353),
                            },
                        })
                        .await
                        .context(TelegramSnafu)?;
                    message_id.get_or_insert(sent);
                }
                for id in &attachment_ids {
                    attachments.payloads.remove(id);
                }
                message_id.expect(
                    "nonempty validated attachment IDs always produce at least one Telegram send",
                )
            }
        };
        Ok(MessageView {
            id: message_id,
            sender: "You".to_owned(),
            body: text,
            timestamp: "now".to_owned(),
            direction: MessageDirection::Outgoing,
            delivery: DeliveryState::Sent,
            reply_to,
            details: MessageDetails {
                thread_root,
                saved_peer,
                ..MessageDetails::default()
            },
        })
    }
}

fn outgoing_message(
    record: &OutgoingRecord<'_>,
    id: MessageId,
    delivery: DeliveryState,
) -> MessageView {
    MessageView {
        id,
        sender: "You".to_owned(),
        body: record.text.to_owned(),
        timestamp: "now".to_owned(),
        direction: MessageDirection::Outgoing,
        delivery,
        reply_to: record.reply_to,
        details: MessageDetails {
            entities: record.entities.to_vec(),
            thread_root: record.thread_root,
            saved_peer: record.saved_peer,
            ..MessageDetails::default()
        },
    }
}
