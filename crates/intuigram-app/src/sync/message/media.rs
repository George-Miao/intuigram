use intuigram_lib::{MediaLocator, MediaSource, MediaThumbnail};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub(super) struct StoredMediaLocator {
    dc_id: i32,
    source: StoredMediaSource,
    name: String,
    mime_type: String,
    size: usize,
    thumbnails: Vec<StoredMediaThumbnail>,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum StoredMediaSource {
    Photo {
        id: i64,
        access_hash: i64,
        file_reference: Vec<u8>,
    },
    Document {
        id: i64,
        access_hash: i64,
        file_reference: Vec<u8>,
    },
}

#[derive(Deserialize, Serialize)]
struct StoredMediaThumbnail {
    kind: String,
    size: usize,
}

pub(super) fn stored_media_locator(locator: &MediaLocator) -> StoredMediaLocator {
    StoredMediaLocator {
        dc_id: locator.dc_id,
        source: match &locator.source {
            MediaSource::Photo {
                id,
                access_hash,
                file_reference,
            } => StoredMediaSource::Photo {
                id: *id,
                access_hash: *access_hash,
                file_reference: file_reference.clone(),
            },
            MediaSource::Document {
                id,
                access_hash,
                file_reference,
            } => StoredMediaSource::Document {
                id: *id,
                access_hash: *access_hash,
                file_reference: file_reference.clone(),
            },
        },
        name: locator.name.clone(),
        mime_type: locator.mime_type.clone(),
        size: locator.size,
        thumbnails: locator
            .thumbnails
            .iter()
            .map(|thumbnail| StoredMediaThumbnail {
                kind: thumbnail.kind.clone(),
                size: thumbnail.size,
            })
            .collect(),
    }
}

pub(super) fn cached_media_locator(locator: StoredMediaLocator) -> MediaLocator {
    MediaLocator {
        dc_id: locator.dc_id,
        source: match locator.source {
            StoredMediaSource::Photo {
                id,
                access_hash,
                file_reference,
            } => MediaSource::Photo {
                id,
                access_hash,
                file_reference,
            },
            StoredMediaSource::Document {
                id,
                access_hash,
                file_reference,
            } => MediaSource::Document {
                id,
                access_hash,
                file_reference,
            },
        },
        name: locator.name,
        mime_type: locator.mime_type,
        size: locator.size,
        thumbnails: locator
            .thumbnails
            .into_iter()
            .map(|thumbnail| MediaThumbnail {
                kind: thumbnail.kind,
                size: thumbnail.size,
            })
            .collect(),
    }
}
