use compio::runtime::ResumeUnwind;

use super::*;

impl Backend {
    pub(super) async fn load_avatar(&mut self, avatar: AvatarRef) -> Result<Option<InlineImage>> {
        let key = avatar_cache_key(avatar);
        let cache = self.media_cache.clone();
        let cached = compio::runtime::spawn_blocking({
            let key = key.clone();
            move || cache.get(intuigram_media::CacheKind::Thumbnail, &key)
        })
        .await
        .resume_unwind()
        .expect("an awaited avatar cache read cannot be cancelled")
        .context(MediaCacheSnafu)?;
        if let Some(bytes) = cached
            && let Some(image) = decode_cached_preview(&bytes)
        {
            return Ok(Some(image));
        }
        let Some(media) = self
            .client
            .download_avatar(avatar)
            .await
            .context(TelegramSnafu)?
        else {
            return Ok(None);
        };
        let cache = self.media_cache.clone();
        compio::runtime::spawn_blocking(move || {
            let avatar = intuigram_media::decode_preview(&media.bytes).ok();
            if let Some(avatar) = &avatar {
                cache.put(
                    intuigram_media::CacheKind::Thumbnail,
                    &key,
                    &encode_cached_preview(avatar),
                )?;
            }
            Ok::<_, intuigram_media::CacheError>(avatar)
        })
        .await
        .resume_unwind()
        .expect("an awaited blocking avatar task cannot be cancelled")
        .context(MediaCacheSnafu)
    }

    pub(super) async fn load_media_preview(
        &mut self,
        chat: ChatId,
        message: MessageId,
        locator: Option<&intuigram_lib::MediaLocator>,
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
        let cache = self.media_cache.clone();
        let retained = compio::runtime::spawn_blocking(move || {
            crate::offline_media::load(&cache, chat, message)
        })
        .await
        .resume_unwind()
        .expect("an awaited retained-media read cannot be cancelled")
        .context(MediaCacheSnafu)?;
        if let Some(media) = retained {
            return Ok(intuigram_media::decode_preview(&media.bytes).ok());
        }
        let media = self
            .client
            .download_media_preview(chat, message, locator)
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
        locator: Option<&intuigram_lib::MediaLocator>,
    ) -> Result<DownloadView> {
        let cache = self.media_cache.clone();
        let retained = compio::runtime::spawn_blocking(move || {
            crate::offline_media::load(&cache, chat, message)
        })
        .await
        .resume_unwind()
        .expect("an awaited retained-media read cannot be cancelled")
        .context(MediaCacheSnafu)?;
        let media = match retained {
            Some(media) => media,
            None => self
                .client
                .download_media(chat, message, locator)
                .await
                .context(TelegramSnafu)?,
        };
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

    pub(super) async fn cache_media_offline(
        &mut self,
        target: intuigram_lib::OfflineMediaTarget,
        locator: Option<&intuigram_lib::MediaLocator>,
    ) -> Result<()> {
        let cache = self.media_cache.clone();
        let cached = compio::runtime::spawn_blocking({
            let cache = cache.clone();
            move || crate::offline_media::load(&cache, target.chat, target.message)
        })
        .await
        .resume_unwind()
        .expect("an awaited retained-media read cannot be cancelled")
        .context(MediaCacheSnafu)?;
        if cached.is_some() {
            return Ok(());
        }
        let media = self
            .client
            .download_media(target.chat, target.message, locator)
            .await
            .context(TelegramSnafu)?;
        compio::runtime::spawn_blocking(move || {
            crate::offline_media::retain(&cache, target.chat, target.message, &media)
        })
        .await
        .resume_unwind()
        .expect("an awaited offline-media write cannot be cancelled")
        .context(MediaCacheSnafu)
    }
}

fn cache_key(chat: ChatId, message: MessageId) -> intuigram_media::CacheKey {
    intuigram_media::CacheKey::new(format!("preview-v2:{}:{}", chat.0, message.0))
}

fn avatar_cache_key(avatar: AvatarRef) -> intuigram_media::CacheKey {
    intuigram_media::CacheKey::new(format!("avatar-v2:{}:{}", avatar.peer.0, avatar.id.0))
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
