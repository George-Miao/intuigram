use intuigram_app::{TextEntity, TextEntityKind};

use super::super::model::shared;
use super::{Error, Result};

pub(super) fn entities(values: &[TextEntity]) -> Result<Vec<shared::TextEntity>> {
    values.iter().map(entity).collect()
}

fn entity(value: &TextEntity) -> Result<shared::TextEntity> {
    Ok(shared::TextEntity {
        offset: u32::try_from(value.offset).map_err(|_| Error::NumericOverflow)?,
        length: u32::try_from(value.length).map_err(|_| Error::NumericOverflow)?,
        kind: match &value.kind {
            TextEntityKind::Bold => shared::TextEntityKind::Bold,
            TextEntityKind::Italic => shared::TextEntityKind::Italic,
            TextEntityKind::Underline => shared::TextEntityKind::Underline,
            TextEntityKind::Strike => shared::TextEntityKind::Strike,
            TextEntityKind::Code => shared::TextEntityKind::Code,
            TextEntityKind::Pre { language } => shared::TextEntityKind::Pre {
                language: language.clone(),
            },
            TextEntityKind::Spoiler => shared::TextEntityKind::Spoiler,
            TextEntityKind::Url => shared::TextEntityKind::Url,
            TextEntityKind::TextUrl { url } => shared::TextEntityKind::TextUrl { url: url.clone() },
            TextEntityKind::Semantic => shared::TextEntityKind::Semantic,
            TextEntityKind::CustomEmoji { document_id } => shared::TextEntityKind::CustomEmoji {
                document_id: *document_id,
            },
        },
    })
}

pub(super) const fn attachment_kind(kind: intuigram_app::AttachmentKind) -> shared::AttachmentKind {
    match kind {
        intuigram_app::AttachmentKind::Photo => shared::AttachmentKind::Photo,
        intuigram_app::AttachmentKind::Video => shared::AttachmentKind::Video,
        intuigram_app::AttachmentKind::File => shared::AttachmentKind::File,
    }
}

pub(super) const fn upload_kind(kind: intuigram_app::RichMediaUploadKind) -> shared::UploadKind {
    match kind {
        intuigram_app::RichMediaUploadKind::Photo => shared::UploadKind::Photo,
        intuigram_app::RichMediaUploadKind::Video => shared::UploadKind::Video,
        intuigram_app::RichMediaUploadKind::File => shared::UploadKind::File,
        intuigram_app::RichMediaUploadKind::Animation => shared::UploadKind::Animation,
        intuigram_app::RichMediaUploadKind::Sticker => shared::UploadKind::Sticker,
        intuigram_app::RichMediaUploadKind::CustomEmoji => shared::UploadKind::CustomEmoji,
        intuigram_app::RichMediaUploadKind::Voice => shared::UploadKind::Voice,
        intuigram_app::RichMediaUploadKind::VideoNote => shared::UploadKind::VideoNote,
    }
}

pub(super) const fn library_kind(
    kind: intuigram_telegram::MediaLibraryKind,
) -> shared::LibraryKind {
    match kind {
        intuigram_telegram::MediaLibraryKind::Stickers => shared::LibraryKind::Stickers,
        intuigram_telegram::MediaLibraryKind::Gifs => shared::LibraryKind::Gifs,
        intuigram_telegram::MediaLibraryKind::CustomEmoji => shared::LibraryKind::CustomEmoji,
    }
}
