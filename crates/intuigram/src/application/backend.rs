pub(super) struct MessageSend {
    pub(super) chat: ChatId,
    pub(super) text: String,
    pub(super) entities: Vec<TextEntity>,
    pub(super) link_preview: bool,
    pub(super) reply_to: Option<MessageId>,
    pub(super) thread_root: Option<MessageId>,
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
            text,
            attachments,
        })
    }

    pub(super) async fn save_draft(
        &mut self,
        chat: ChatId,
        thread_root: Option<MessageId>,
        text: String,
        reply_to: Option<MessageId>,
    ) -> Result<()> {
        self.store
            .save_draft(intuigram_store::StoredDraft {
                chat_id: chat.0,
                thread_root: thread_root.map(|message| message.0),
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
    ) -> Result<()> {
        self.store
            .save_selection(intuigram_store::StoredSelection {
                folder_id: folder,
                chat_id: chat.map(|chat| chat.0),
                anchor_message_id: message.map(|message| message.0),
            })
            .context(AccountDatabaseSnafu)?
            .await
            .context(AccountDatabaseSnafu)
    }

    pub(super) async fn load_chat(
        &mut self,
        chat: ChatId,
    ) -> Result<(Vec<MessageView>, Vec<MessageView>)> {
        let messages = self
            .client
            .complete_history(chat, 100)
            .await
            .context(TelegramSnafu)?;
        let pinned_messages = self
            .client
            .pinned_messages(chat, 100)
            .await
            .context(TelegramSnafu)?;
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
            )
            .context(AccountDatabaseSnafu)?
            .await
            .context(AccountDatabaseSnafu)?;
        Ok((messages, pinned_messages))
    }

    pub(super) async fn load_selected_chat(
        &mut self,
        chat: ChatId,
        selection: Option<SelectionView>,
    ) -> Result<Option<AdapterEvent>> {
        if let Some(selection) = selection {
            self.save_selection(selection.folder, selection.chat, selection.message)
                .await?;
        }
        match self.load_chat(chat).await {
            Ok((messages, pinned_messages)) => Ok(Some(AdapterEvent::ChatLoaded {
                chat,
                messages,
                pinned_messages,
            })),
            Err(error) => history_failure_event(chat, None, error),
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
        let OutgoingRecord {
            chat,
            local_id,
            text,
            entities,
            reply_to,
            thread_root,
            delivery,
        } = record;
        self.store
            .save_messages(vec![encode_stored_message(
                chat,
                &MessageView {
                    id: local_id,
                    sender: "You".to_owned(),
                    body: text.to_owned(),
                    timestamp: "now".to_owned(),
                    direction: MessageDirection::Outgoing,
                    delivery,
                    reply_to,
                    details: MessageDetails {
                        entities: entities.to_vec(),
                        thread_root,
                        ..MessageDetails::default()
                    },
                },
            )])
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
            attachment_ids,
            random_id,
        } = request;
        let message_id = {
            let Self {
                client,
                next_local_message_id,
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
                        random_id,
                        schedule_date: None,
                    })
                    .await
                    .context(TelegramSnafu)?;
            } else {
                let payloads = attachment_ids
                    .iter()
                    .filter_map(|id| {
                        attachments
                            .payloads
                            .get(id)
                            .cloned()
                            .map(|payload| (*id, payload))
                    })
                    .collect::<Vec<_>>();
                for (index, (_, payload)) in payloads.iter().enumerate() {
                    let upload = match payload {
                        AttachmentPayload::Image { mime_type, bytes } => {
                            intuigram_telegram::Upload {
                                name: "clipboard.png".to_owned(),
                                mime_type: mime_type.clone(),
                                bytes: bytes.clone(),
                                kind: intuigram_telegram::UploadKind::Photo,
                            }
                        }
                        AttachmentPayload::File { path, kind } => intuigram_telegram::Upload {
                            name: path.file_name().map_or_else(
                                || "attachment".to_owned(),
                                |name| name.to_string_lossy().into_owned(),
                            ),
                            mime_type: mime_type_for_path(path),
                            bytes: compio::fs::read(path)
                                .await
                                .context(ReadAttachmentSnafu { path: path.clone() })?,
                            kind: match kind {
                                AttachmentKind::Photo => intuigram_telegram::UploadKind::Photo,
                                AttachmentKind::Video => intuigram_telegram::UploadKind::Video,
                                AttachmentKind::File => intuigram_telegram::UploadKind::File,
                            },
                        },
                    };
                    client
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
                            ids: intuigram_telegram::UploadIds {
                                file: derived_random_id(random_id, index, 0x4649_4c45),
                                message: derived_random_id(random_id, index, 0x4d45_5353),
                            },
                        })
                        .await
                        .context(TelegramSnafu)?;
                }
                for id in &attachment_ids {
                    attachments.payloads.remove(id);
                }
            }
            *next_local_message_id -= 1;
            *next_local_message_id
        };
        Ok(MessageView {
            id: MessageId(message_id),
            sender: "You".to_owned(),
            body: text,
            timestamp: "now".to_owned(),
            direction: MessageDirection::Outgoing,
            delivery: DeliveryState::Sent,
            reply_to,
            details: MessageDetails {
                thread_root,
                ..MessageDetails::default()
            },
        })
    }
}
use super::*;
