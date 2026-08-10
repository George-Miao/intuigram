use intuigram_lib::{TextEntity, TextEntityKind};

use super::super::model::shared;
use super::{InvalidSnafu, Result};

pub(super) fn entities(values: &[shared::TextEntity]) -> Vec<TextEntity> {
    values
        .iter()
        .map(|value| TextEntity {
            offset: value.offset as usize,
            length: value.length as usize,
            kind: match &value.kind {
                shared::TextEntityKind::Bold => TextEntityKind::Bold,
                shared::TextEntityKind::Italic => TextEntityKind::Italic,
                shared::TextEntityKind::Underline => TextEntityKind::Underline,
                shared::TextEntityKind::Strike => TextEntityKind::Strike,
                shared::TextEntityKind::Code => TextEntityKind::Code,
                shared::TextEntityKind::Pre { language } => TextEntityKind::Pre {
                    language: language.clone(),
                },
                shared::TextEntityKind::Spoiler => TextEntityKind::Spoiler,
                shared::TextEntityKind::Url => TextEntityKind::Url,
                shared::TextEntityKind::TextUrl { url } => {
                    TextEntityKind::TextUrl { url: url.clone() }
                }
                shared::TextEntityKind::Semantic => TextEntityKind::Semantic,
                shared::TextEntityKind::CustomEmoji { document_id } => {
                    TextEntityKind::CustomEmoji {
                        document_id: *document_id,
                    }
                }
            },
        })
        .collect()
}

pub(super) fn media(
    values: &[intuigram_store::OutboxMedia],
    position: shared::MediaPosition,
    kind: intuigram_telegram::UploadKind,
) -> Result<intuigram_telegram::Upload> {
    let index = usize::try_from(position.0).map_err(|_| super::Error::Invalid {
        reason: "media position exceeds this platform's address space",
    })?;
    let value = values.get(index).ok_or(super::Error::Invalid {
        reason: "media position is outside the retained byte set",
    })?;
    if value.bytes.is_empty() {
        return InvalidSnafu {
            reason: "retained media is empty",
        }
        .fail();
    }
    Ok(intuigram_telegram::Upload {
        name: value.file_name.clone(),
        mime_type: value.mime_type.clone(),
        bytes: value.bytes.clone(),
        kind,
    })
}

pub(super) const fn attachment_kind(
    kind: shared::AttachmentKind,
) -> intuigram_telegram::UploadKind {
    match kind {
        shared::AttachmentKind::Photo => intuigram_telegram::UploadKind::Photo,
        shared::AttachmentKind::Video => intuigram_telegram::UploadKind::Video,
        shared::AttachmentKind::File => intuigram_telegram::UploadKind::File,
    }
}

pub(super) const fn upload_kind(kind: shared::UploadKind) -> intuigram_telegram::UploadKind {
    match kind {
        shared::UploadKind::Photo => intuigram_telegram::UploadKind::Photo,
        shared::UploadKind::Video => intuigram_telegram::UploadKind::Video,
        shared::UploadKind::File => intuigram_telegram::UploadKind::File,
        shared::UploadKind::Animation => intuigram_telegram::UploadKind::Animation,
        shared::UploadKind::Sticker => intuigram_telegram::UploadKind::Sticker,
        shared::UploadKind::CustomEmoji => intuigram_telegram::UploadKind::CustomEmoji,
        shared::UploadKind::Voice => intuigram_telegram::UploadKind::Voice,
        shared::UploadKind::VideoNote => intuigram_telegram::UploadKind::VideoNote,
    }
}

pub(super) const fn library_kind(
    kind: shared::LibraryKind,
) -> intuigram_telegram::MediaLibraryKind {
    match kind {
        shared::LibraryKind::Stickers => intuigram_telegram::MediaLibraryKind::Stickers,
        shared::LibraryKind::Gifs => intuigram_telegram::MediaLibraryKind::Gifs,
        shared::LibraryKind::CustomEmoji => intuigram_telegram::MediaLibraryKind::CustomEmoji,
    }
}
