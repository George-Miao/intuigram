use super::*;

/// One rich text Message submission.
pub struct TextSend {
    /// Destination Chat.
    pub chat: ChatId,

    /// Plain text after composer markup is removed.
    pub text: String,

    /// Telegram rich-text entities using UTF-16 offsets.
    pub entities: Vec<TextEntity>,

    /// Whether Telegram may generate a webpage preview.
    pub link_preview: bool,

    /// Direct reply target.
    pub reply_to: Option<MessageId>,

    /// Active Thread root.
    pub thread_root: Option<MessageId>,

    /// Stable idempotency token for this operation.
    pub random_id: i64,
}

/// One uploaded photo, video, or file submission.
pub struct UploadSend {
    /// Destination Chat.
    pub chat: ChatId,

    /// Complete upload payload.
    pub upload: Upload,

    /// Plain caption text.
    pub caption: String,

    /// Caption entities using UTF-16 offsets.
    pub entities: Vec<TextEntity>,

    /// Direct reply target.
    pub reply_to: Option<MessageId>,

    /// Active Thread root.
    pub thread_root: Option<MessageId>,

    /// Stable upload and Message idempotency identifiers.
    pub ids: UploadIds,
}

impl Client {
    /// Sends a rich text Message, optionally as a reply.
    pub async fn send_text(&mut self, request: TextSend) -> Result<()> {
        let TextSend {
            chat,
            text,
            entities,
            link_preview,
            reply_to,
            thread_root,
            random_id,
        } = request;
        let peer = self.peers.resolve(chat)?;
        let reply_to = reply_to
            .or(thread_root)
            .map(|message| {
                let reply_to_msg_id =
                    i32::try_from(message.0).map_err(|_| Error::InvalidMessageId {
                        message_id: message.0,
                    })?;
                Ok(tl::types::InputReplyToMessage {
                    reply_to_msg_id,
                    top_msg_id: thread_root
                        .filter(|root| *root != message)
                        .map(|root| {
                            i32::try_from(root.0)
                                .map_err(|_| Error::InvalidMessageId { message_id: root.0 })
                        })
                        .transpose()?,
                    reply_to_peer_id: None,
                    quote_text: None,
                    quote_entities: None,
                    quote_offset: None,
                    monoforum_peer_id: None,
                    todo_item_id: None,
                    poll_option: None,
                }
                .into())
            })
            .transpose()?;
        let entities = serialize_entities(entities)?;
        self.connection
            .invoke(&tl::functions::messages::SendMessage {
                no_webpage: !link_preview,
                silent: false,
                background: false,
                clear_draft: true,
                noforwards: false,
                update_stickersets_order: false,
                invert_media: false,
                allow_paid_floodskip: false,
                peer,
                reply_to,
                message: text,
                random_id,
                reply_markup: None,
                entities,
                schedule_date: None,
                schedule_repeat_period: None,
                send_as: None,
                quick_reply_shortcut: None,
                effect: None,
                allow_paid_stars: None,
                suggested_post: None,
                rich_message: None,
            })
            .await
            .context(InvokeSnafu)?;
        Ok(())
    }

    /// Uploads and sends one photo or generic document.
    pub async fn send_upload(&mut self, request: UploadSend) -> Result<()> {
        let UploadSend {
            chat,
            upload,
            caption,
            entities,
            reply_to,
            thread_root,
            ids,
        } = request;
        const PART_BYTES: usize = 512 * 1024;
        const BIG_FILE_BYTES: usize = 10 * 1024 * 1024;

        let peer = self.peers.resolve(chat)?;
        let part_count = upload.bytes.len().div_ceil(PART_BYTES);
        let part_count = i32::try_from(part_count).map_err(|_| Error::InvalidMessageId {
            message_id: i64::try_from(upload.bytes.len()).unwrap_or(i64::MAX),
        })?;
        let big = upload.bytes.len() > BIG_FILE_BYTES;
        for (part, bytes) in upload.bytes.chunks(PART_BYTES).enumerate() {
            let part = i32::try_from(part)
                .expect("an in-memory upload cannot exceed Telegram's signed part index");
            let accepted = if big {
                self.connection
                    .invoke(&tl::functions::upload::SaveBigFilePart {
                        file_id: ids.file,
                        file_part: part,
                        file_total_parts: part_count,
                        bytes: bytes.to_vec(),
                    })
                    .await
                    .context(InvokeSnafu)?
            } else {
                self.connection
                    .invoke(&tl::functions::upload::SaveFilePart {
                        file_id: ids.file,
                        file_part: part,
                        bytes: bytes.to_vec(),
                    })
                    .await
                    .context(InvokeSnafu)?
            };
            if !accepted {
                return UploadPartRejectedSnafu { part }.fail();
            }
        }
        let input_file = if big {
            tl::types::InputFileBig {
                id: ids.file,
                parts: part_count,
                name: upload.name.clone(),
            }
            .into()
        } else {
            tl::types::InputFile {
                id: ids.file,
                parts: part_count,
                name: upload.name.clone(),
                md5_checksum: format!("{:x}", md5::compute(&upload.bytes)),
            }
            .into()
        };
        let media = uploaded_media(upload, input_file);
        let entities = serialize_entities(entities)?;
        self.connection
            .invoke(&tl::functions::messages::SendMedia {
                silent: false,
                background: false,
                clear_draft: true,
                noforwards: false,
                update_stickersets_order: false,
                invert_media: false,
                allow_paid_floodskip: false,
                peer,
                reply_to: input_reply_to(reply_to, thread_root)?,
                media,
                message: caption,
                random_id: ids.message,
                reply_markup: None,
                entities,
                schedule_date: None,
                schedule_repeat_period: None,
                send_as: None,
                quick_reply_shortcut: None,
                effect: None,
                allow_paid_stars: None,
                suggested_post: None,
            })
            .await
            .context(InvokeSnafu)?;
        Ok(())
    }

    /// Returns a direct IPv4 endpoint advertised by Telegram for a data center.
    #[must_use]
    pub fn data_center_endpoint(&self, dc_id: i32) -> Option<SocketAddr> {
        self.data_centers.get(&dc_id).copied()
    }
}

fn uploaded_media(upload: Upload, input_file: tl::enums::InputFile) -> tl::enums::InputMedia {
    match upload.kind {
        UploadKind::Photo => tl::types::InputMediaUploadedPhoto {
            spoiler: false,
            live_photo: false,
            file: input_file,
            stickers: None,
            ttl_seconds: None,
            video: None,
        }
        .into(),
        UploadKind::Video
        | UploadKind::Voice
        | UploadKind::VideoNote
        | UploadKind::Animation
        | UploadKind::Sticker
        | UploadKind::CustomEmoji
        | UploadKind::File => {
            let mut attributes = vec![
                tl::types::DocumentAttributeFilename {
                    file_name: upload.name,
                }
                .into(),
            ];
            attributes.extend(upload_attributes(upload.kind));
            tl::types::InputMediaUploadedDocument {
                nosound_video: upload.kind == UploadKind::Animation,
                force_file: false,
                spoiler: false,
                file: input_file,
                thumb: None,
                mime_type: upload.mime_type,
                attributes,
                stickers: None,
                video_cover: None,
                video_timestamp: None,
                ttl_seconds: None,
            }
            .into()
        }
    }
}

fn upload_attributes(kind: UploadKind) -> Vec<tl::enums::DocumentAttribute> {
    match kind {
        UploadKind::Video | UploadKind::VideoNote => vec![
            tl::types::DocumentAttributeVideo {
                round_message: kind == UploadKind::VideoNote,
                supports_streaming: true,
                nosound: false,
                duration: 0.0,
                w: 0,
                h: 0,
                preload_prefix_size: None,
                video_start_ts: None,
                video_codec: None,
            }
            .into(),
        ],
        UploadKind::Voice => vec![
            tl::types::DocumentAttributeAudio {
                voice: true,
                duration: 0,
                title: None,
                performer: None,
                waveform: None,
            }
            .into(),
        ],
        UploadKind::Animation => vec![tl::enums::DocumentAttribute::Animated],
        UploadKind::Sticker => vec![
            tl::types::DocumentAttributeSticker {
                mask: false,
                alt: String::new(),
                stickerset: tl::enums::InputStickerSet::Empty,
                mask_coords: None,
            }
            .into(),
        ],
        UploadKind::CustomEmoji => vec![
            tl::types::DocumentAttributeCustomEmoji {
                free: false,
                text_color: false,
                alt: String::new(),
                stickerset: tl::enums::InputStickerSet::Empty,
            }
            .into(),
        ],
        UploadKind::Photo | UploadKind::File => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_uploads_include_streaming_video_metadata() {
        let media = uploaded_media(
            Upload {
                name: "clip.mp4".to_owned(),
                mime_type: "video/mp4".to_owned(),
                bytes: Vec::new(),
                kind: UploadKind::Video,
            },
            tl::types::InputFile {
                id: 1,
                parts: 1,
                name: "clip.mp4".to_owned(),
                md5_checksum: String::new(),
            }
            .into(),
        );

        let tl::enums::InputMedia::UploadedDocument(media) = media else {
            panic!("video should use uploaded-document media")
        };
        assert!(media.attributes.iter().any(|attribute| matches!(
            attribute,
            tl::enums::DocumentAttribute::Video(video) if video.supports_streaming
        )));
    }

    #[test]
    fn note_and_library_uploads_use_telegram_specific_attributes() {
        let attributes = [
            (UploadKind::Voice, "voice"),
            (UploadKind::VideoNote, "video-note"),
            (UploadKind::Animation, "animation"),
            (UploadKind::Sticker, "sticker"),
            (UploadKind::CustomEmoji, "custom-emoji"),
        ]
        .map(|(kind, label)| (label, upload_attributes(kind)));

        assert!(matches!(
            &attributes[0].1[0],
            tl::enums::DocumentAttribute::Audio(audio) if audio.voice
        ));
        assert!(matches!(
            &attributes[1].1[0],
            tl::enums::DocumentAttribute::Video(video) if video.round_message
        ));
        assert!(matches!(
            &attributes[2].1[0],
            tl::enums::DocumentAttribute::Animated
        ));
        assert!(matches!(
            &attributes[3].1[0],
            tl::enums::DocumentAttribute::Sticker(_)
        ));
        assert!(matches!(
            &attributes[4].1[0],
            tl::enums::DocumentAttribute::CustomEmoji(_)
        ));
    }
}
