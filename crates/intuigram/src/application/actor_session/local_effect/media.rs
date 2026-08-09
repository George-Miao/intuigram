use std::cell::RefCell;

use compio::runtime::ResumeUnwind;
use intuigram_app::{AdapterEvent, DownloadView, InlineImage, MediaPreviewView};
use snafu::ResultExt;

use super::super::super::{MediaCacheSnafu, Result, SaveDownloadSnafu};
use super::State;

pub(super) async fn cached_preview(
    state: &RefCell<State>,
    chat: intuigram_app::ChatId,
    message: intuigram_app::MessageId,
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
    Ok(cached.and_then(|bytes| decode_cached_preview(&bytes)))
}

pub(super) async fn finish_preview(
    state: &RefCell<State>,
    chat: intuigram_app::ChatId,
    message: intuigram_app::MessageId,
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

pub(super) async fn finish_download(
    state: &RefCell<State>,
    chat: intuigram_app::ChatId,
    message: intuigram_app::MessageId,
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
    chat: intuigram_app::ChatId,
    message: intuigram_app::MessageId,
) -> intuigram_media::CacheKey {
    intuigram_media::CacheKey::new(format!("preview-v2:{}:{}", chat.0, message.0))
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
    use intuigram_app::{ChatId, MessageId};

    use super::cache_key;

    #[test]
    fn high_density_previews_do_not_reuse_legacy_cache_entries() {
        assert_ne!(
            cache_key(ChatId(10), MessageId(20)),
            intuigram_media::CacheKey::new("10:20")
        );
    }
}
