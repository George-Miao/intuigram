use intuigram_store::{OutboxMedia, OutboxPayloadV1};

use super::super::model::send::{MessageSend, TextMessage};
use super::super::model::shared::{MediaPosition, UploadKind};
use super::super::model::{Command, Destination, PreparedCommand};
use super::{conversion, validate};

#[test]
fn envelope_validation_rejects_a_changed_random_id() {
    let command = text_command(41);
    let mut payload = payload(41);

    assert!(validate(&payload, &command).is_ok());
    payload.random_id = 42;
    assert!(validate(&payload, &command).is_err());
}

#[test]
fn retained_media_position_resolves_exact_admitted_bytes() {
    let media = vec![OutboxMedia::new(
        "detail.png".to_owned(),
        "image/png".to_owned(),
        vec![1, 2, 3],
    )];

    let upload = conversion::media(
        &media,
        MediaPosition(0),
        conversion::upload_kind(UploadKind::Photo),
    )
    .expect("valid position should resolve");

    assert_eq!(upload.name, "detail.png");
    assert_eq!(upload.bytes, [1, 2, 3]);
    assert!(
        conversion::media(
            &media,
            MediaPosition(1),
            conversion::upload_kind(UploadKind::Photo)
        )
        .is_err()
    );
}

fn text_command(random_id: i64) -> PreparedCommand {
    PreparedCommand::new(
        Destination {
            chat_id: 7,
            thread_root: None,
            saved_peer: None,
        },
        Some(random_id),
        Command::Text(MessageSend::new(
            -1,
            None,
            TextMessage {
                text: "hello".to_owned(),
                entities: Vec::new(),
                link_preview: true,
                attachments: Vec::new(),
            },
        )),
    )
}

fn payload(random_id: i64) -> OutboxPayloadV1 {
    OutboxPayloadV1 {
        chat_id: 7,
        thread_root: None,
        saved_peer: None,
        local_message_id: Some(-1),
        random_id,
        content: Vec::new(),
    }
}
