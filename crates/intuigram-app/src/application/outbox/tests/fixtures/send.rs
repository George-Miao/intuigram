use super::super::super::model::send::{
    Contact, LibraryMedia, MessageSend, Poll, TextMessage, Venue,
};
use super::super::super::model::shared::{
    AttachmentKind, GeoPoint, LibraryKind, MediaPosition, PreparedAttachment, PreparedMedia,
    UploadKind,
};
use super::super::super::model::{Command, PreparedCommand};
use super::{prepared, text_entities};

pub(in crate::application::outbox::tests) fn send_commands() -> Vec<PreparedCommand> {
    vec![
        prepared(
            Some(i64::MIN + 7),
            Command::Text(MessageSend::new(
                -41,
                Some(42),
                TextMessage {
                    text: "hi 🚀".to_owned(),
                    entities: text_entities(),
                    link_preview: false,
                    attachments: vec![
                        PreparedAttachment::new(MediaPosition(0), AttachmentKind::Photo),
                        PreparedAttachment::new(MediaPosition(u32::MAX), AttachmentKind::File),
                    ],
                },
            )),
        ),
        prepared(
            Some(2),
            Command::Poll(MessageSend::new(
                -42,
                None,
                Poll {
                    question: "tea?".to_owned(),
                    options: vec!["yes".to_owned(), "no".to_owned()],
                },
            )),
        ),
        prepared(
            Some(3),
            Command::Library(MessageSend::new(
                -43,
                None,
                LibraryMedia {
                    kind: LibraryKind::CustomEmoji,
                    document_id: 44,
                    access_hash: -45,
                    file_reference: vec![0, 127, 255],
                },
            )),
        ),
        prepared(
            Some(4),
            Command::Contact(MessageSend::new(
                -44,
                None,
                Contact {
                    phone: "+86 123".to_owned(),
                    first_name: "Ada".to_owned(),
                    last_name: "Lovelace".to_owned(),
                },
            )),
        ),
        prepared(
            Some(5),
            Command::File(MessageSend::new(
                -45,
                None,
                PreparedMedia::new(MediaPosition(7), UploadKind::Animation),
            )),
        ),
        prepared(
            Some(6),
            Command::Recording(MessageSend::new(
                -46,
                None,
                PreparedMedia::new(MediaPosition(8), UploadKind::Voice),
            )),
        ),
        prepared(
            Some(7),
            Command::StaticLocation(MessageSend::new(
                -47,
                None,
                GeoPoint {
                    latitude_microdegrees: 31_230_400,
                    longitude_microdegrees: 121_473_700,
                },
            )),
        ),
        prepared(
            Some(8),
            Command::Venue(MessageSend::new(
                -48,
                None,
                Venue {
                    point: GeoPoint {
                        latitude_microdegrees: -90_000_000,
                        longitude_microdegrees: 180_000_000,
                    },
                    title: "Cafe".to_owned(),
                    address: "1 Main St".to_owned(),
                    provider: "provider".to_owned(),
                    venue_id: "id".to_owned(),
                    venue_type: "food/cafe".to_owned(),
                },
            )),
        ),
    ]
}
