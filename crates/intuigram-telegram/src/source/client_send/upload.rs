use super::*;

impl Client {
    /// Uploads and sends one photo or generic document.
    pub async fn send_upload(&mut self, request: UploadSend) -> Result<MessageId> {
        self.send_upload_with_policy(request, InvocationPolicy::WaitForFlood)
            .await
    }

    /// Uploads and sends one photo or document using the invocation policy.
    pub async fn send_upload_with_policy(
        &mut self,
        request: UploadSend,
        policy: InvocationPolicy,
    ) -> Result<MessageId> {
        let UploadSend {
            chat,
            upload,
            caption,
            entities,
            reply_to,
            thread_root,
            monoforum_peer,
            ids,
        } = request;
        let peer = self.peers.resolve(chat)?;
        let monoforum_peer = monoforum_peer
            .map(|peer| self.peers.resolve(peer))
            .transpose()?;
        let media = self
            .upload_media_with_policy(upload, ids.file, policy)
            .await?;
        let entities = serialize_entities(entities)?;
        let updates = self
            .invoke_outbound(
                &tl::functions::messages::SendMedia {
                    silent: false,
                    background: false,
                    clear_draft: true,
                    noforwards: false,
                    update_stickersets_order: false,
                    invert_media: false,
                    allow_paid_floodskip: false,
                    peer,
                    reply_to: input_reply_to(reply_to, thread_root, monoforum_peer)?,
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
                },
                policy,
            )
            .await?;
        sent_message_id(updates, ids.message)
    }

    pub(in crate::source) async fn upload_media_with_policy(
        &mut self,
        upload: Upload,
        file_id: i64,
        policy: InvocationPolicy,
    ) -> Result<tl::enums::InputMedia> {
        const PART_BYTES: usize = 512 * 1024;
        const BIG_FILE_BYTES: usize = 10 * 1024 * 1024;

        let part_count = upload.bytes.len().div_ceil(PART_BYTES);
        let part_count = i32::try_from(part_count).map_err(|_| Error::InvalidMessageId {
            message_id: i64::try_from(upload.bytes.len()).unwrap_or(i64::MAX),
        })?;
        let big = upload.bytes.len() > BIG_FILE_BYTES;
        for (part, bytes) in upload.bytes.chunks(PART_BYTES).enumerate() {
            let part = i32::try_from(part)
                .expect("an in-memory upload cannot exceed Telegram's signed part index");
            let accepted = if big {
                self.invoke_outbound(
                    &tl::functions::upload::SaveBigFilePart {
                        file_id,
                        file_part: part,
                        file_total_parts: part_count,
                        bytes: bytes.to_vec(),
                    },
                    policy,
                )
                .await?
            } else {
                self.invoke_outbound(
                    &tl::functions::upload::SaveFilePart {
                        file_id,
                        file_part: part,
                        bytes: bytes.to_vec(),
                    },
                    policy,
                )
                .await?
            };
            if !accepted {
                return UploadPartRejectedSnafu { part }.fail();
            }
        }
        let input_file = if big {
            tl::types::InputFileBig {
                id: file_id,
                parts: part_count,
                name: upload.name.clone(),
            }
            .into()
        } else {
            tl::types::InputFile {
                id: file_id,
                parts: part_count,
                name: upload.name.clone(),
                md5_checksum: format!("{:x}", md5::compute(&upload.bytes)),
            }
            .into()
        };
        Ok(uploaded_media(upload, input_file))
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
