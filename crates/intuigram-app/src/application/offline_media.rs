use intuigram_lib::{ChatId, MessageId};
use intuigram_media::{CacheError, CacheKey, CacheKind, CacheOwner, MediaCache};
use intuigram_telegram::DownloadedMedia;

const MAGIC: &[u8] = b"intuigram-media-v1\0";

pub(super) fn owner(chat: ChatId) -> CacheOwner {
    CacheOwner::new(format!("chat:{}", chat.0))
}

pub(super) fn key(chat: ChatId, message: MessageId) -> CacheKey {
    CacheKey::new(format!("original-v1:{}:{}", chat.0, message.0))
}

pub(super) fn load(
    cache: &MediaCache,
    chat: ChatId,
    message: MessageId,
) -> Result<Option<DownloadedMedia>, CacheError> {
    cache
        .get_retained(CacheKind::Media, &owner(chat), &key(chat, message))
        .map(|bytes| bytes.and_then(|bytes| decode(&bytes)))
}

pub(super) fn retain(
    cache: &MediaCache,
    chat: ChatId,
    message: MessageId,
    media: &DownloadedMedia,
) -> Result<(), CacheError> {
    cache.put_retained(
        CacheKind::Media,
        &owner(chat),
        &key(chat, message),
        &encode(media),
    )
}

fn encode(media: &DownloadedMedia) -> Vec<u8> {
    let name = media.name.as_bytes();
    let mime_type = media.mime_type.as_bytes();
    let mut output = Vec::with_capacity(
        MAGIC
            .len()
            .saturating_add(8)
            .saturating_add(name.len())
            .saturating_add(mime_type.len())
            .saturating_add(media.bytes.len()),
    );
    output.extend_from_slice(MAGIC);
    let name_length = u32::try_from(name.len())
        .expect("a platform filename cannot approach four gigabytes of UTF-8 metadata");
    let mime_length = u32::try_from(mime_type.len())
        .expect("an Internet media type cannot approach four gigabytes of UTF-8 metadata");
    output.extend_from_slice(&name_length.to_le_bytes());
    output.extend_from_slice(&mime_length.to_le_bytes());
    output.extend_from_slice(name);
    output.extend_from_slice(mime_type);
    output.extend_from_slice(&media.bytes);
    output
}

fn decode(bytes: &[u8]) -> Option<DownloadedMedia> {
    let payload = bytes.strip_prefix(MAGIC)?;
    let name_length =
        usize::try_from(u32::from_le_bytes(payload.get(..4)?.try_into().ok()?)).ok()?;
    let mime_length =
        usize::try_from(u32::from_le_bytes(payload.get(4..8)?.try_into().ok()?)).ok()?;
    let name_end = 8_usize.checked_add(name_length)?;
    let mime_end = name_end.checked_add(mime_length)?;
    Some(DownloadedMedia {
        name: String::from_utf8(payload.get(8..name_end)?.to_vec()).ok()?,
        mime_type: String::from_utf8(payload.get(name_end..mime_end)?.to_vec()).ok()?,
        bytes: payload.get(mime_end..)?.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn retained_original_round_trips_without_telegram_types_in_the_cache_crate() {
        let temporary = tempdir().expect("temporary cache should be created");
        let cache = MediaCache::new(temporary.path(), 0);
        let media = DownloadedMedia {
            name: "photo.png".to_owned(),
            mime_type: "image/png".to_owned(),
            bytes: vec![1, 2, 3],
        };

        retain(&cache, ChatId(7), MessageId(9), &media).expect("original media should be retained");
        let restored = load(&cache, ChatId(7), MessageId(9))
            .expect("retained media should load")
            .expect("retained media should exist");

        assert_eq!(restored.name, media.name);
        assert_eq!(restored.mime_type, media.mime_type);
        assert_eq!(restored.bytes, media.bytes);
    }
}
