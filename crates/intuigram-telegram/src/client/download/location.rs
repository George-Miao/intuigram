use super::super::*;

pub(super) struct DownloadLocation {
    pub(super) location: tl::enums::InputFileLocation,
    pub(super) dc_id: i32,
    pub(super) name: String,
    pub(super) mime_type: String,
    pub(super) size: usize,
}

pub(super) fn download_location(
    media: tl::enums::MessageMedia,
    message: MessageId,
    maximum_bytes: Option<usize>,
) -> Result<Option<DownloadLocation>> {
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
            let full_size = valid_size(document.size)?;
            let thumbnail = maximum_bytes.and_then(|maximum| {
                document
                    .thumbs
                    .as_deref()
                    .and_then(|sizes| largest_photo_size(sizes, maximum))
            });
            let (thumb_size, size, mime_type) = match (maximum_bytes, thumbnail) {
                (_, Some((thumb_size, size))) => (
                    thumb_size,
                    valid_size(i64::from(size))?,
                    "image/jpeg".to_owned(),
                ),
                (Some(maximum), None) if full_size > maximum => return Ok(None),
                _ => (String::new(), full_size, document.mime_type),
            };
            Ok(Some(DownloadLocation {
                location: tl::types::InputDocumentFileLocation {
                    id: document.id,
                    access_hash: document.access_hash,
                    file_reference: document.file_reference,
                    thumb_size,
                }
                .into(),
                dc_id: document.dc_id,
                name,
                mime_type,
                size,
            }))
        }
        tl::enums::MessageMedia::Photo(media) => {
            let Some(tl::enums::Photo::Photo(photo)) = media.photo else {
                return DownloadMediaUnavailableSnafu {
                    message_id: message.0,
                }
                .fail();
            };
            let maximum = maximum_bytes.unwrap_or(usize::MAX);
            let (thumb_size, size) = largest_photo_size(&photo.sizes, maximum).context(
                DownloadMediaUnavailableSnafu {
                    message_id: message.0,
                },
            )?;
            Ok(Some(DownloadLocation {
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
            }))
        }
        _ => DownloadMediaUnavailableSnafu {
            message_id: message.0,
        }
        .fail(),
    }
}

pub(super) fn download_locator(
    locator: &MediaLocator,
    maximum_bytes: Option<usize>,
) -> Option<DownloadLocation> {
    let thumbnail = match &locator.source {
        MediaSource::Photo { .. } => locator
            .thumbnails
            .iter()
            .filter(|thumbnail| maximum_bytes.is_none_or(|maximum| thumbnail.size <= maximum))
            .max_by_key(|thumbnail| thumbnail.size),
        MediaSource::Document { .. } => maximum_bytes.and_then(|maximum| {
            locator
                .thumbnails
                .iter()
                .filter(|thumbnail| thumbnail.size <= maximum)
                .max_by_key(|thumbnail| thumbnail.size)
        }),
    };
    let (thumb_size, size, mime_type) = match (&locator.source, maximum_bytes, thumbnail) {
        (_, _, Some(thumbnail)) => (
            thumbnail.kind.clone(),
            thumbnail.size,
            "image/jpeg".to_owned(),
        ),
        (MediaSource::Document { .. }, Some(maximum), None) if locator.size > maximum => {
            return None;
        }
        (MediaSource::Photo { .. }, Some(_), None) => return None,
        (MediaSource::Photo { .. }, None, None) => return None,
        (_, _, None) => (String::new(), locator.size, locator.mime_type.clone()),
    };
    let location = match &locator.source {
        MediaSource::Photo {
            id,
            access_hash,
            file_reference,
        } => tl::types::InputPhotoFileLocation {
            id: *id,
            access_hash: *access_hash,
            file_reference: file_reference.clone(),
            thumb_size,
        }
        .into(),
        MediaSource::Document {
            id,
            access_hash,
            file_reference,
        } => tl::types::InputDocumentFileLocation {
            id: *id,
            access_hash: *access_hash,
            file_reference: file_reference.clone(),
            thumb_size,
        }
        .into(),
    };
    Some(DownloadLocation {
        location,
        dc_id: locator.dc_id,
        name: locator.name.clone(),
        mime_type,
        size,
    })
}

pub(super) fn largest_photo_size(
    sizes: &[tl::enums::PhotoSize],
    maximum_bytes: usize,
) -> Option<(String, i32)> {
    sizes
        .iter()
        .filter_map(|size| match size {
            tl::enums::PhotoSize::Size(size) => Some((size.r#type.clone(), size.size)),
            tl::enums::PhotoSize::Progressive(size) => {
                size.sizes.last().map(|bytes| (size.r#type.clone(), *bytes))
            }
            _ => None,
        })
        .filter(|(_, bytes)| usize::try_from(*bytes).is_ok_and(|bytes| bytes <= maximum_bytes))
        .max_by_key(|(_, bytes)| *bytes)
}

fn valid_size(size: i64) -> Result<usize> {
    usize::try_from(size).map_err(|_| Error::InvalidDownloadSize { size })
}

pub(super) fn expired_file_reference(error: &Error) -> bool {
    matches!(
        error,
        Error::Invoke {
            source: InvocationError::Rpc { message, .. },
        } if message.starts_with("FILE_REFERENCE_")
    )
}
