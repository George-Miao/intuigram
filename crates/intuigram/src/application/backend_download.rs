use compio::runtime::ResumeUnwind;

use super::*;

impl Backend {
    pub(super) async fn load_media_preview(
        &mut self,
        chat: ChatId,
        message: MessageId,
    ) -> Result<Option<InlineImage>> {
        let key = cache_key(chat, message);
        let cache = self.media_cache.clone();
        let cached = compio::runtime::spawn_blocking({
            let key = key.clone();
            move || cache.get(intuigram_media::CacheKind::Thumbnail, &key)
        })
        .await
        .resume_unwind()
        .expect("an awaited cache read cannot be cancelled")
        .context(MediaCacheSnafu)?;
        if let Some(bytes) = cached
            && let Some(image) = decode_cached_preview(&bytes)
        {
            return Ok(Some(image));
        }
        let media = self
            .client
            .download_media_preview(chat, message)
            .await
            .context(TelegramSnafu)?;
        let Some(media) = media else {
            return Ok(None);
        };
        let cache = self.media_cache.clone();
        compio::runtime::spawn_blocking(move || {
            let preview = intuigram_media::decode_preview(&media.bytes).ok();
            if let Some(preview) = &preview {
                cache.put(
                    intuigram_media::CacheKind::Thumbnail,
                    &key,
                    &encode_cached_preview(preview),
                )?;
            }
            Ok::<_, intuigram_media::CacheError>(preview)
        })
        .await
        .resume_unwind()
        .expect("an awaited blocking media-preview task cannot be cancelled")
        .context(MediaCacheSnafu)
    }

    pub(super) async fn download_media(
        &mut self,
        chat: ChatId,
        message: MessageId,
        destination: Option<String>,
    ) -> Result<DownloadView> {
        let media = self
            .client
            .download_media(chat, message)
            .await
            .context(TelegramSnafu)?;
        let name = media.name;
        let mime_type = media.mime_type;
        let bytes = media.bytes;
        let (bytes, preview) = compio::runtime::spawn_blocking(move || {
            let preview = intuigram_media::decode_preview(&bytes).ok();
            (bytes, preview)
        })
        .await
        .resume_unwind()
        .expect("an awaited blocking media-preview task cannot be cancelled");
        let path = match destination {
            Some(destination) => self.downloads.save_to(destination, bytes).await,
            None => self.downloads.save(&name, bytes).await,
        }
        .context(SaveDownloadSnafu)?;
        let reveal_only = intuigram_media::open_disposition(&path, Some(&mime_type))
            == intuigram_media::OpenDisposition::RevealWithLaunchWarning;
        let id = self.downloaded.register(path.clone());
        Ok(DownloadView {
            chat,
            id,
            path: path.display().to_string(),
            reveal_only,
            message,
            preview,
        })
    }
}

fn cache_key(chat: ChatId, message: MessageId) -> intuigram_media::CacheKey {
    intuigram_media::CacheKey::new(format!("{}:{}", chat.0, message.0))
}

fn encode_cached_preview(image: &InlineImage) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(image.rgba().len().saturating_add(4));
    bytes.extend_from_slice(&image.width().to_le_bytes());
    bytes.extend_from_slice(&image.height().to_le_bytes());
    bytes.extend_from_slice(image.rgba());
    bytes
}

fn decode_cached_preview(bytes: &[u8]) -> Option<InlineImage> {
    let [width_low, width_high, height_low, height_high, rgba @ ..] = bytes else {
        return None;
    };
    InlineImage::from_rgba(
        u16::from_le_bytes([*width_low, *width_high]),
        u16::from_le_bytes([*height_low, *height_high]),
        rgba.to_vec(),
    )
}
