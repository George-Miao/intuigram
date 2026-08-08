use super::*;

impl Backend {
    pub(super) async fn execute_rich_media(
        &mut self,
        effect: Effect,
        random_id: Option<i64>,
    ) -> Result<AdapterEvent> {
        match effect {
            Effect::BrowseRichMedia { kind } => self.browse_rich_media(kind).await,
            Effect::SendLibraryMedia {
                chat,
                item,
                local_id,
                reply_to,
                thread_root,
            } => {
                let body = self
                    .media_library
                    .entries
                    .get(&item)
                    .map_or_else(|| "[media]".to_owned(), |entry| entry.label.clone());
                self.send_library_rich_media(LibrarySend {
                    record: RichMediaRecord {
                        chat,
                        local_id,
                        body,
                        reply_to,
                        thread_root,
                    },
                    item,
                    random_id: rich_media_random_id(random_id),
                })
                .await
            }
            Effect::SendRichMediaFile { .. } | Effect::RecordRichMedia { .. } => {
                Err(Error::UnpreparedRichMedia)
            }
            Effect::SendContact {
                chat,
                phone,
                first_name,
                last_name,
                local_id,
                reply_to,
                thread_root,
            } => {
                self.send_contact_rich_media(ContactSend {
                    record: RichMediaRecord {
                        chat,
                        local_id,
                        body: format!("[Contact] {first_name} {last_name}"),
                        reply_to,
                        thread_root,
                    },
                    phone,
                    first_name,
                    last_name,
                    random_id: rich_media_random_id(random_id),
                })
                .await
            }
            _ => unreachable!("the effect dispatcher only routes rich-media effects"),
        }
    }

    pub(super) async fn execute_prepared_rich_media(
        &mut self,
        effect: Effect,
        random_id: Option<i64>,
        prepared: PreparedRichMedia,
    ) -> Result<Option<AdapterEvent>> {
        let record = match effect {
            Effect::SendRichMediaFile {
                chat,
                kind,
                local_id,
                reply_to,
                thread_root,
                ..
            } => RichMediaRecord {
                chat,
                local_id,
                body: format!("[{kind:?}] {}", prepared.name),
                reply_to,
                thread_root,
            },
            Effect::RecordRichMedia {
                chat,
                kind,
                local_id,
                reply_to,
                thread_root,
                ..
            } => RichMediaRecord {
                chat,
                local_id,
                body: format!("[{kind:?}]"),
                reply_to,
                thread_root,
            },
            _ => return Err(Error::UnpreparedRichMedia),
        };
        let event = self
            .send_file_rich_media(FileSend {
                record,
                upload: intuigram_telegram::Upload {
                    name: prepared.name,
                    mime_type: prepared.mime_type,
                    bytes: prepared.bytes,
                    kind: upload_kind(prepared.kind),
                },
                random_id: rich_media_random_id(random_id),
            })
            .await?;
        Ok(Some(event))
    }

    pub(super) async fn browse_rich_media(
        &mut self,
        kind: RichMediaLibraryKind,
    ) -> Result<AdapterEvent> {
        let result = self.client.browse_media(library_kind(kind), "", 50).await;
        Ok(match result {
            Ok(entries) => AdapterEvent::RichMediaLibraryReady {
                kind,
                items: self.media_library.register(entries),
            },
            Err(source) if source.is_connection_failure() => {
                return Err(Error::Telegram { source });
            }
            Err(error) => AdapterEvent::RichMediaLibraryFailed(error.to_string()),
        })
    }

    pub(super) async fn send_library_rich_media(
        &mut self,
        request: LibrarySend,
    ) -> Result<AdapterEvent> {
        self.persist_rich_media(&request.record, DeliveryState::Pending)
            .await?;
        let result = match self.media_library.entries.get(&request.item).cloned() {
            Some(entry) => self
                .client
                .send_library_media(
                    request.record.chat,
                    &entry,
                    request.record.reply_to,
                    request.record.thread_root,
                    request.random_id,
                )
                .await
                .map_err(|source| Error::Telegram { source }),
            None => Err(Error::MediaLibraryItemUnavailable {
                item: request.item.0,
            }),
        };
        self.finish_rich_media(request.record, result).await
    }

    pub(super) async fn send_file_rich_media(&mut self, request: FileSend) -> Result<AdapterEvent> {
        self.persist_rich_media(&request.record, DeliveryState::Pending)
            .await?;
        let result = self.upload_rich_media(&request).await;
        self.finish_rich_media(request.record, result).await
    }

    pub(super) async fn send_contact_rich_media(
        &mut self,
        request: ContactSend,
    ) -> Result<AdapterEvent> {
        self.persist_rich_media(&request.record, DeliveryState::Pending)
            .await?;
        let result = self
            .client
            .send_contact(intuigram_telegram::ContactCardSend {
                chat: request.record.chat,
                phone_number: request.phone,
                first_name: request.first_name,
                last_name: request.last_name,
                reply_to: request.record.reply_to,
                thread_root: request.record.thread_root,
                random_id: request.random_id,
            })
            .await
            .map_err(|source| Error::Telegram { source });
        self.finish_rich_media(request.record, result).await
    }

    async fn upload_rich_media(&mut self, request: &FileSend) -> Result<MessageId> {
        self.client
            .send_upload(intuigram_telegram::UploadSend {
                chat: request.record.chat,
                upload: intuigram_telegram::Upload {
                    name: request.upload.name.clone(),
                    mime_type: request.upload.mime_type.clone(),
                    bytes: request.upload.bytes.clone(),
                    kind: request.upload.kind,
                },
                caption: String::new(),
                entities: Vec::new(),
                reply_to: request.record.reply_to,
                thread_root: request.record.thread_root,
                ids: intuigram_telegram::UploadIds {
                    file: derived_random_id(request.random_id, 0, 0x4649_4c45),
                    message: derived_random_id(request.random_id, 0, 0x4d45_5353),
                },
            })
            .await
            .context(TelegramSnafu)
    }

    async fn finish_rich_media(
        &mut self,
        record: RichMediaRecord,
        result: Result<MessageId>,
    ) -> Result<AdapterEvent> {
        if let Err(Error::Telegram { source }) = &result
            && source.is_connection_failure()
        {
            return result.map(|_| unreachable!());
        }
        match &result {
            Ok(server_id) => {
                self.acknowledge_outgoing(
                    OutgoingRecord {
                        chat: record.chat,
                        local_id: record.local_id,
                        text: &record.body,
                        entities: &[],
                        reply_to: record.reply_to,
                        thread_root: record.thread_root,
                        delivery: DeliveryState::Sent,
                    },
                    *server_id,
                )
                .await?;
            }
            Err(_) => {
                self.persist_rich_media(&record, DeliveryState::Failed)
                    .await?
            }
        }
        Ok(match result {
            Ok(server_id) => AdapterEvent::RichMediaAcknowledged {
                chat: record.chat,
                local_id: record.local_id,
                server_id,
            },
            Err(error) => AdapterEvent::RichMediaFailed {
                chat: record.chat,
                local_id: record.local_id,
                reason: error.to_string(),
            },
        })
    }

    async fn persist_rich_media(
        &mut self,
        record: &RichMediaRecord,
        delivery: DeliveryState,
    ) -> Result<()> {
        self.persist_outgoing(OutgoingRecord {
            chat: record.chat,
            local_id: record.local_id,
            text: &record.body,
            entities: &[],
            reply_to: record.reply_to,
            thread_root: record.thread_root,
            delivery,
        })
        .await
    }
}

#[derive(Clone)]
pub(super) struct RichMediaRecord {
    pub(super) chat: ChatId,
    pub(super) local_id: MessageId,
    pub(super) body: String,
    pub(super) reply_to: Option<MessageId>,
    pub(super) thread_root: Option<MessageId>,
}

pub(super) struct LibrarySend {
    pub(super) record: RichMediaRecord,
    pub(super) item: RichMediaItemId,
    pub(super) random_id: i64,
}

pub(super) struct FileSend {
    pub(super) record: RichMediaRecord,
    pub(super) upload: intuigram_telegram::Upload,
    pub(super) random_id: i64,
}

pub(super) struct ContactSend {
    pub(super) record: RichMediaRecord,
    pub(super) phone: String,
    pub(super) first_name: String,
    pub(super) last_name: String,
    pub(super) random_id: i64,
}

fn library_kind(kind: RichMediaLibraryKind) -> MediaLibraryKind {
    match kind {
        RichMediaLibraryKind::Stickers => MediaLibraryKind::Stickers,
        RichMediaLibraryKind::Gifs => MediaLibraryKind::Gifs,
        RichMediaLibraryKind::CustomEmoji => MediaLibraryKind::CustomEmoji,
    }
}

pub(super) fn upload_kind(kind: RichMediaUploadKind) -> UploadKind {
    match kind {
        RichMediaUploadKind::Photo => UploadKind::Photo,
        RichMediaUploadKind::Video => UploadKind::Video,
        RichMediaUploadKind::File => UploadKind::File,
        RichMediaUploadKind::Animation => UploadKind::Animation,
        RichMediaUploadKind::Sticker => UploadKind::Sticker,
        RichMediaUploadKind::CustomEmoji => UploadKind::CustomEmoji,
        RichMediaUploadKind::Voice => UploadKind::Voice,
        RichMediaUploadKind::VideoNote => UploadKind::VideoNote,
    }
}

fn rich_media_random_id(random_id: Option<i64>) -> i64 {
    random_id.expect("every queued rich-media send has an idempotency token")
}
