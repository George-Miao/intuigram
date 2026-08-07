use super::*;

const DOWNLOAD_PART_BYTES: i32 = 512 * 1024;

struct DownloadLocation {
    location: tl::enums::InputFileLocation,
    dc_id: i32,
    name: String,
    mime_type: String,
    size: usize,
}

impl Client {
    /// Fetches one Message's full photo or document bytes from Telegram.
    pub async fn download_media(
        &mut self,
        chat: ChatId,
        message: MessageId,
    ) -> Result<DownloadedMedia> {
        let media = self.message_media(chat, message).await?;
        let download = download_location(media, message)?;
        if download.dc_id != self.dc_id {
            return self.download_from_data_center(download).await;
        }
        self.download_from_current_connection(download).await
    }

    async fn download_from_data_center(
        &mut self,
        download: DownloadLocation,
    ) -> Result<DownloadedMedia> {
        let dc_id = download.dc_id;
        let endpoint = self
            .data_centers
            .get(&dc_id)
            .copied()
            .context(MediaDataCenterUnavailableSnafu { dc_id })?;
        let exported = self
            .connection
            .invoke(&tl::functions::auth::ExportAuthorization { dc_id })
            .await
            .context(InvokeSnafu)?;
        let tl::enums::auth::ExportedAuthorization::Authorization(exported) = exported;
        let (mut media_client, _) =
            Client::connect_new(dc_id, endpoint, self.credentials.clone()).await?;
        media_client
            .connection
            .invoke(&tl::functions::auth::ImportAuthorization {
                id: exported.id,
                bytes: exported.bytes,
            })
            .await
            .context(InvokeSnafu)?;
        media_client
            .download_from_current_connection(download)
            .await
    }

    async fn download_from_current_connection(
        &mut self,
        download: DownloadLocation,
    ) -> Result<DownloadedMedia> {
        let mut bytes = Vec::with_capacity(download.size);
        while bytes.len() < download.size {
            let offset = i64::try_from(bytes.len())
                .map_err(|_| Error::InvalidDownloadSize { size: i64::MAX })?;
            let file = self
                .connection
                .invoke(&tl::functions::upload::GetFile {
                    precise: false,
                    cdn_supported: false,
                    location: download.location.clone(),
                    offset,
                    limit: DOWNLOAD_PART_BYTES,
                })
                .await
                .context(InvokeSnafu)?;
            let tl::enums::upload::File::File(part) = file else {
                return DownloadCdnRedirectSnafu.fail();
            };
            if part.bytes.is_empty() {
                break;
            }
            bytes.extend_from_slice(&part.bytes);
        }
        bytes.truncate(download.size);
        if bytes.len() != download.size {
            return IncompleteDownloadSnafu {
                expected: download.size,
                actual: bytes.len(),
            }
            .fail();
        }
        Ok(DownloadedMedia {
            name: download.name,
            mime_type: download.mime_type,
            bytes,
        })
    }

    pub(super) async fn message_media(
        &mut self,
        chat: ChatId,
        message: MessageId,
    ) -> Result<tl::enums::MessageMedia> {
        let id = i32::try_from(message.0).map_err(|_| Error::InvalidMessageId {
            message_id: message.0,
        })?;
        let input = vec![tl::types::InputMessageId { id }.into()];
        let peer = self.peers.resolve(chat)?;
        let response = match peer {
            tl::enums::InputPeer::Channel(channel) => self
                .connection
                .invoke(&tl::functions::channels::GetMessages {
                    channel: tl::types::InputChannel {
                        channel_id: channel.channel_id,
                        access_hash: channel.access_hash,
                    }
                    .into(),
                    id: input,
                })
                .await
                .context(InvokeSnafu)?,
            _ => self
                .connection
                .invoke(&tl::functions::messages::GetMessages { id: input })
                .await
                .context(InvokeSnafu)?,
        };
        let (messages, chats, users) = message_parts(response);
        self.update_peer_cache(&chats, &users);
        messages
            .into_iter()
            .find(|candidate| candidate.id() == id)
            .and_then(|candidate| match candidate {
                tl::enums::Message::Message(message) => message.media,
                tl::enums::Message::Empty(_) | tl::enums::Message::Service(_) => None,
            })
            .context(DownloadMessageUnavailableSnafu {
                message_id: message.0,
            })
    }
}

fn download_location(
    media: tl::enums::MessageMedia,
    message: MessageId,
) -> Result<DownloadLocation> {
    match media {
        tl::enums::MessageMedia::Document(media) => {
            let Some(tl::enums::Document::Document(document)) = media.document else {
                return DownloadMediaUnavailableSnafu {
                    message_id: message.0,
                }
                .fail();
            };
            let name = document
                .attributes
                .iter()
                .find_map(|attribute| match attribute {
                    tl::enums::DocumentAttribute::Filename(filename) => {
                        Some(filename.file_name.clone())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| format!("file-{}", document.id));
            Ok(DownloadLocation {
                location: tl::types::InputDocumentFileLocation {
                    id: document.id,
                    access_hash: document.access_hash,
                    file_reference: document.file_reference,
                    thumb_size: String::new(),
                }
                .into(),
                dc_id: document.dc_id,
                name,
                mime_type: document.mime_type,
                size: valid_size(document.size)?,
            })
        }
        tl::enums::MessageMedia::Photo(media) => {
            let Some(tl::enums::Photo::Photo(photo)) = media.photo else {
                return DownloadMediaUnavailableSnafu {
                    message_id: message.0,
                }
                .fail();
            };
            let (thumb_size, size) =
                largest_photo_size(&photo.sizes).context(DownloadMediaUnavailableSnafu {
                    message_id: message.0,
                })?;
            Ok(DownloadLocation {
                location: tl::types::InputPhotoFileLocation {
                    id: photo.id,
                    access_hash: photo.access_hash,
                    file_reference: photo.file_reference,
                    thumb_size,
                }
                .into(),
                dc_id: photo.dc_id,
                name: format!("photo-{}.jpg", photo.id),
                mime_type: "image/jpeg".to_owned(),
                size: valid_size(i64::from(size))?,
            })
        }
        _ => DownloadMediaUnavailableSnafu {
            message_id: message.0,
        }
        .fail(),
    }
}

fn largest_photo_size(sizes: &[tl::enums::PhotoSize]) -> Option<(String, i32)> {
    sizes
        .iter()
        .filter_map(|size| match size {
            tl::enums::PhotoSize::Size(size) => Some((size.r#type.clone(), size.size)),
            tl::enums::PhotoSize::Progressive(size) => {
                size.sizes.last().map(|bytes| (size.r#type.clone(), *bytes))
            }
            _ => None,
        })
        .max_by_key(|(_, bytes)| *bytes)
}

fn valid_size(size: i64) -> Result<usize> {
    usize::try_from(size).map_err(|_| Error::InvalidDownloadSize { size })
}
