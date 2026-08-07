use super::*;

mod document;
mod metadata;
mod poll;
mod service;
mod web_page;

use document::normalize_document_media;
pub(super) use metadata::{normalize_forward, normalize_reactions};
use poll::normalize_poll;
pub(crate) use service::service_event_description;
use web_page::normalize_web_page;

pub(super) fn normalize_media(media: &tl::enums::MessageMedia) -> MediaCard {
    match media {
        tl::enums::MessageMedia::Photo(media) => card(
            MediaKind::Photo,
            "Photo",
            if media.spoiler { "spoiler" } else { "image" },
            Vec::new(),
            media.photo.as_ref().and_then(photo_remote_id),
        ),
        tl::enums::MessageMedia::Document(media) => normalize_document_media(media),
        tl::enums::MessageMedia::WebPage(media) => normalize_web_page(&media.webpage),
        tl::enums::MessageMedia::Poll(media) => normalize_poll(media),
        tl::enums::MessageMedia::Contact(media) => card(
            MediaKind::Contact,
            "Contact",
            [media.first_name.as_str(), media.last_name.as_str()]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" "),
            vec![format!("phone {}", media.phone_number)],
            None,
        ),
        tl::enums::MessageMedia::Geo(media) => card(
            MediaKind::Location,
            "Location",
            geo_description(&media.geo),
            Vec::new(),
            None,
        ),
        tl::enums::MessageMedia::Venue(media) => card(
            MediaKind::Venue,
            media.title.clone(),
            media.address.clone(),
            vec![geo_description(&media.geo)],
            None,
        ),
        tl::enums::MessageMedia::Dice(media) => card(
            MediaKind::Dice,
            media.emoticon.clone(),
            format!("result {}", media.value),
            Vec::new(),
            None,
        ),
        tl::enums::MessageMedia::Empty | tl::enums::MessageMedia::Unsupported => card(
            MediaKind::Unsupported,
            "Unsupported Content",
            "Telegram media constructor is not available in this client",
            Vec::new(),
            None,
        ),
        tl::enums::MessageMedia::GeoLive(_)
        | tl::enums::MessageMedia::Game(_)
        | tl::enums::MessageMedia::Invoice(_)
        | tl::enums::MessageMedia::Story(_)
        | tl::enums::MessageMedia::Giveaway(_)
        | tl::enums::MessageMedia::GiveawayResults(_)
        | tl::enums::MessageMedia::PaidMedia(_)
        | tl::enums::MessageMedia::ToDo(_)
        | tl::enums::MessageMedia::VideoStream(_) => card(
            MediaKind::Specialized,
            "Specialized Telegram content",
            "open Details for available metadata",
            Vec::new(),
            None,
        ),
    }
}

pub(super) fn card(
    kind: MediaKind,
    title: impl Into<String>,
    description: impl Into<String>,
    details: Vec<String>,
    remote_id: Option<String>,
) -> MediaCard {
    MediaCard {
        kind,
        title: title.into(),
        description: description.into(),
        details,
        poll: None,
        remote_id,
    }
}

/// Normalizes one serialized current-layer Telegram media constructor into an
/// informative Intuigram-owned card, including unsupported constructors.
pub fn normalize_serialized_media(bytes: &[u8]) -> Result<MediaCard> {
    let media = tl::enums::MessageMedia::from_bytes(bytes).context(DecodeMediaSnafu)?;
    Ok(normalize_media(&media))
}

fn geo_description(geo: &tl::enums::GeoPoint) -> String {
    match geo {
        tl::enums::GeoPoint::Point(point) => format!("{:.6}, {:.6}", point.lat, point.long),
        tl::enums::GeoPoint::Empty => "coordinates unavailable".to_owned(),
    }
}

pub(super) fn photo_remote_id(photo: &tl::enums::Photo) -> Option<String> {
    match photo {
        tl::enums::Photo::Photo(photo) => Some(photo.id.to_string()),
        tl::enums::Photo::Empty(_) => None,
    }
}

pub(super) fn media_card_fallback(card: &MediaCard) -> String {
    if card.description.is_empty() {
        format!("[{}]", card.title)
    } else {
        format!("[{}] {}", card.title, card.description)
    }
}

pub(super) fn nonnegative_u32(value: Option<i32>) -> Option<u32> {
    value.and_then(|value| u32::try_from(value).ok())
}

pub(super) fn format_timestamp(timestamp: i32) -> String {
    local_datetime(timestamp).map_or_else(
        || "--:--".to_owned(),
        |local| format!("{:02}:{:02}", local.hour(), local.minute()),
    )
}

pub(super) fn format_date(timestamp: i32) -> String {
    local_datetime(timestamp).map_or_else(String::new, |local| local.date().to_string())
}

fn local_datetime(timestamp: i32) -> Option<time::OffsetDateTime> {
    let Ok(utc) = time::OffsetDateTime::from_unix_timestamp(i64::from(timestamp)) else {
        return None;
    };
    let offset = time::UtcOffset::local_offset_at(utc).unwrap_or(time::UtcOffset::UTC);
    Some(utc.to_offset(offset))
}

pub(super) fn user_display_name(user: &tl::types::User) -> String {
    let display_name = [user.first_name.as_deref(), user.last_name.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
    if display_name.is_empty() {
        user.username.clone().unwrap_or_else(|| user.id.to_string())
    } else {
        display_name
    }
}
