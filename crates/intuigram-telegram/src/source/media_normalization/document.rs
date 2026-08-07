use super::*;

pub(super) fn normalize_document_media(media: &tl::types::MessageMediaDocument) -> MediaCard {
    let Some(tl::enums::Document::Document(document)) = media.document.as_ref() else {
        return card(
            MediaKind::Unsupported,
            "Unavailable file",
            "Telegram did not include document metadata",
            Vec::new(),
            None,
        );
    };
    let filename = document
        .attributes
        .iter()
        .find_map(|attribute| match attribute {
            tl::enums::DocumentAttribute::Filename(attribute) => Some(attribute.file_name.clone()),
            _ => None,
        });
    let kind = document_kind(media, document);
    card(
        kind,
        filename.unwrap_or_else(|| format!("{kind:?}")),
        format!("{} · {} bytes", document.mime_type, document.size),
        document_details(document),
        Some(document.id.to_string()),
    )
}

fn document_kind(
    media: &tl::types::MessageMediaDocument,
    document: &tl::types::Document,
) -> MediaKind {
    if media.round {
        MediaKind::VideoNote
    } else if media.voice {
        MediaKind::Voice
    } else if document.attributes.iter().any(|attribute| {
        matches!(
            attribute,
            tl::enums::DocumentAttribute::Sticker(_) | tl::enums::DocumentAttribute::CustomEmoji(_)
        )
    }) {
        MediaKind::Sticker
    } else if has_attribute(document, |attribute| {
        matches!(attribute, tl::enums::DocumentAttribute::Animated)
    }) {
        MediaKind::Animation
    } else if media.video
        || has_attribute(document, |attribute| {
            matches!(attribute, tl::enums::DocumentAttribute::Video(_))
        })
    {
        MediaKind::Video
    } else if has_attribute(document, |attribute| {
        matches!(attribute, tl::enums::DocumentAttribute::Audio(_))
    }) {
        MediaKind::Audio
    } else {
        MediaKind::File
    }
}

fn has_attribute(
    document: &tl::types::Document,
    predicate: impl Fn(&tl::enums::DocumentAttribute) -> bool,
) -> bool {
    document.attributes.iter().any(predicate)
}

fn document_details(document: &tl::types::Document) -> Vec<String> {
    document
        .attributes
        .iter()
        .filter_map(|attribute| match attribute {
            tl::enums::DocumentAttribute::Audio(audio) => {
                Some(format_duration(f64::from(audio.duration)))
            }
            tl::enums::DocumentAttribute::Video(video) => Some(format!(
                "{}×{} · {}",
                video.w,
                video.h,
                format_duration(video.duration)
            )),
            tl::enums::DocumentAttribute::Sticker(sticker) => {
                Some(format!("sticker {}", sticker.alt))
            }
            _ => None,
        })
        .collect()
}

fn format_duration(seconds: f64) -> String {
    let seconds = seconds.max(0.0) as u64;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}
