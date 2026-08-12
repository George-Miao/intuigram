use std::cell::RefCell;

use compio::runtime::ResumeUnwind;
use intuigram_lib::{
    AdapterEvent, AvatarView, DownloadView, InlineImage, MediaPreviewView, OfflineMediaFailure,
    OfflineMediaTarget,
};
use snafu::ResultExt;

use super::super::super::{MediaCacheSnafu, Result, SaveDownloadSnafu};
use super::State;

pub(super) async fn cached_preview(
    state: &RefCell<State>,
    chat: intuigram_lib::ChatId,
    message: intuigram_lib::MessageId,
) -> Result<Option<InlineImage>> {
    let cache = state.borrow().media_cache.clone();
    let key = cache_key(chat, message);
    let cached = compio::runtime::spawn_blocking(move || {
        cache.get(intuigram_media::CacheKind::Thumbnail, &key)
    })
    .await
    .resume_unwind()
    .expect("an awaited cache read cannot be cancelled")
    .context(MediaCacheSnafu)?;
    if let Some(image) = cached.and_then(|bytes| decode_cached_preview(&bytes)) {
        return Ok(Some(image));
    }
    let cache = state.borrow().media_cache.clone();
    let original = compio::runtime::spawn_blocking(move || {
        let Some(media) = crate::offline_media::load(&cache, chat, message)? else {
            return Ok(None);
        };
        let preview = intuigram_media::decode_preview(&media.bytes).ok();
        if let Some(preview) = &preview {
            cache.put_retained(
                intuigram_media::CacheKind::Thumbnail,
                &crate::offline_media::owner(chat),
                &cache_key(chat, message),
                &encode_cached_preview(preview),
            )?;
        }
        Ok::<_, intuigram_media::CacheError>(preview)
    })
    .await
    .resume_unwind()
    .expect("an awaited retained preview decode cannot be cancelled")
    .context(MediaCacheSnafu)?;
    Ok(original)
}

pub(super) async fn cached_original(
    state: &RefCell<State>,
    target: OfflineMediaTarget,
) -> Result<Option<intuigram_telegram::DownloadedMedia>> {
    let cache = state.borrow().media_cache.clone();
    compio::runtime::spawn_blocking(move || {
        crate::offline_media::load(&cache, target.chat, target.message)
    })
    .await
    .resume_unwind()
    .expect("an awaited retained-media read cannot be cancelled")
    .context(MediaCacheSnafu)
}

pub(super) async fn cached_avatar(
    state: &RefCell<State>,
    avatar: intuigram_lib::AvatarRef,
) -> Result<Option<InlineImage>> {
    let cache = state.borrow().media_cache.clone();
    let key = avatar_cache_key(avatar);
    let cached = compio::runtime::spawn_blocking(move || {
        cache.get(intuigram_media::CacheKind::Thumbnail, &key)
    })
    .await
    .resume_unwind()
    .expect("an awaited avatar cache read cannot be cancelled")
    .context(MediaCacheSnafu)?;
    Ok(cached.and_then(|bytes| decode_cached_preview(&bytes)))
}

pub(super) async fn finish_preview(
    state: &RefCell<State>,
    chat: intuigram_lib::ChatId,
    message: intuigram_lib::MessageId,
    media: Option<intuigram_telegram::DownloadedMedia>,
) -> Result<AdapterEvent> {
    let Some(media) = media else {
        return Ok(AdapterEvent::MediaPreviewFailed { chat, message });
    };
    let cache = state.borrow().media_cache.clone();
    let key = cache_key(chat, message);
    let preview = compio::runtime::spawn_blocking(move || {
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
    .context(MediaCacheSnafu)?;
    Ok(match preview {
        Some(image) => AdapterEvent::MediaPreviewReady(MediaPreviewView {
            chat,
            message,
            image,
        }),
        None => AdapterEvent::MediaPreviewFailed { chat, message },
    })
}

pub(super) async fn finish_avatar(
    state: &RefCell<State>,
    avatar_ref: intuigram_lib::AvatarRef,
    media: Option<intuigram_telegram::DownloadedMedia>,
) -> Result<AdapterEvent> {
    let Some(media) = media else {
        return Ok(AdapterEvent::AvatarFailed { avatar: avatar_ref });
    };
    let cache = state.borrow().media_cache.clone();
    let key = avatar_cache_key(avatar_ref);
    let avatar = compio::runtime::spawn_blocking(move || {
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
    .context(MediaCacheSnafu)?;
    Ok(match avatar {
        Some(image) => AdapterEvent::AvatarReady(AvatarView {
            avatar: avatar_ref,
            image,
        }),
        None => AdapterEvent::AvatarFailed { avatar: avatar_ref },
    })
}

pub(super) async fn finish_offline_media(
    state: &RefCell<State>,
    target: OfflineMediaTarget,
    media: Option<intuigram_telegram::DownloadedMedia>,
) -> Result<AdapterEvent> {
    let Some(media) = media else {
        return Ok(AdapterEvent::MediaCacheOfflineFailed(OfflineMediaFailure {
            chat: target.chat,
            message: Some(target.message),
            reason: "Telegram did not return downloadable media".to_owned(),
        }));
    };
    let cache = state.borrow().media_cache.clone();
    compio::runtime::spawn_blocking(move || {
        crate::offline_media::retain(&cache, target.chat, target.message, &media)?;
        if let Ok(preview) = intuigram_media::decode_preview(&media.bytes) {
            cache.put_retained(
                intuigram_media::CacheKind::Thumbnail,
                &crate::offline_media::owner(target.chat),
                &cache_key(target.chat, target.message),
                &encode_cached_preview(&preview),
            )?;
        }
        Ok::<_, intuigram_media::CacheError>(())
    })
    .await
    .resume_unwind()
    .expect("an awaited offline-media cache write cannot be cancelled")
    .context(MediaCacheSnafu)?;
    Ok(AdapterEvent::MediaCachedOffline(target))
}

pub(super) async fn finish_download(
    state: &RefCell<State>,
    chat: intuigram_lib::ChatId,
    message: intuigram_lib::MessageId,
    destination: Option<String>,
    media: intuigram_telegram::DownloadedMedia,
) -> Result<AdapterEvent> {
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
    let downloads = state.borrow().downloads.clone();
    let path = match destination {
        Some(destination) => downloads.save_to(destination, bytes).await,
        None => downloads.save(&name, bytes).await,
    }
    .context(SaveDownloadSnafu)?;
    let reveal_only = intuigram_media::open_disposition(&path, Some(&mime_type))
        == intuigram_media::OpenDisposition::RevealWithLaunchWarning;
    let id = state.borrow_mut().downloaded.register(path.clone());
    Ok(AdapterEvent::DownloadReady {
        chat,
        download: DownloadView {
            chat,
            id,
            path: path.display().to_string(),
            reveal_only,
            message,
            preview,
        },
    })
}

fn cache_key(
    chat: intuigram_lib::ChatId,
    message: intuigram_lib::MessageId,
) -> intuigram_media::CacheKey {
    intuigram_media::CacheKey::new(format!("preview-v2:{}:{}", chat.0, message.0))
}

fn avatar_cache_key(avatar: intuigram_lib::AvatarRef) -> intuigram_media::CacheKey {
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

#[cfg(test)]
mod tests {
    use intuigram_lib::{ChatId, MessageId};

    use super::cache_key;

    #[test]
    fn high_density_previews_do_not_reuse_legacy_cache_entries() {
        assert_ne!(
            cache_key(ChatId(10), MessageId(20)),
            intuigram_media::CacheKey::new("10:20")
        );
    }
}
