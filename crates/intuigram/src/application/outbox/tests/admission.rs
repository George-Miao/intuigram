use intuigram::{Clock, OperationIdSource, OperationProviders};
use intuigram_app::{
    AttachmentId, ChatId, Effect, MessageDetails, MessageDirection, MessageId, MessageView,
};
use intuigram_store::{OutboxOperation, OutboxPayload};

use super::super::model::Command;
use super::super::{admission, codec};
use crate::application::{AttachmentPayload, decode_stored_message};

#[test]
fn message_admission_captures_bytes_and_optimistic_message_atomically() {
    let effect = Effect::SendMessage {
        chat: ChatId(7),
        text: "hello".to_owned(),
        entities: Vec::new(),
        link_preview: true,
        reply_to: Some(MessageId(3)),
        thread_root: Some(MessageId(2)),
        saved_peer: None,
        attachments: vec![AttachmentId(9)],
        local_id: MessageId(-1),
    };

    let admission = admission::prepare(
        &effect,
        stamp(),
        vec![(
            AttachmentId(9),
            AttachmentPayload::Image {
                mime_type: "image/png".to_owned(),
                bytes: vec![1, 2, 3],
            },
        )],
        None,
        None,
    )
    .expect("valid message admission should succeed");

    assert_eq!(admission.operation, OutboxOperation::Send);
    assert!(admission.consume_draft);
    assert_eq!(admission.media[0].bytes, [1, 2, 3]);
    let optimistic = decode_stored_message(
        admission
            .optimistic_message
            .expect("a send should include an optimistic message"),
    );
    assert_eq!(optimistic.id, MessageId(-1));
    assert_eq!(optimistic.body, "hello");
    assert_eq!(optimistic.details.thread_root, Some(MessageId(2)));
    let OutboxPayload::V1(payload) = admission.payload;
    assert_eq!(payload.random_id, 41);
    assert!(matches!(
        codec::decode(&payload.content)
            .expect("admitted semantic command should decode")
            .command(),
        Command::Text(send) if send.content.attachments.len() == 1
    ));
}

#[test]
fn mutation_admission_does_not_rewrite_the_message_before_acknowledgement() {
    let effect = Effect::ReactMessage {
        chat: ChatId(7),
        message: Box::new(MessageView {
            id: MessageId(8),
            sender: "A".to_owned(),
            body: "hello".to_owned(),
            timestamp: "now".to_owned(),
            direction: MessageDirection::Outgoing,
            delivery: intuigram_app::DeliveryState::Sent,
            reply_to: None,
            details: MessageDetails::default(),
        }),
        reaction: "👍".to_owned(),
    };

    let admission = admission::prepare(&effect, stamp(), Vec::new(), None, None)
        .expect("valid mutation admission should succeed");

    assert_eq!(admission.operation, OutboxOperation::Mutation);
    assert!(admission.optimistic_message.is_none());
    assert!(!admission.consume_draft);
}

#[test]
fn missing_selected_attachment_rejects_admission() {
    let effect = Effect::SendMessage {
        chat: ChatId(7),
        text: String::new(),
        entities: Vec::new(),
        link_preview: true,
        reply_to: None,
        thread_root: None,
        saved_peer: None,
        attachments: vec![AttachmentId(9)],
        local_id: MessageId(-1),
    };

    assert!(admission::prepare(&effect, stamp(), Vec::new(), None, None).is_err());
}

fn stamp() -> intuigram::OperationStamp {
    OperationProviders::new(FixedClock, FixedIds)
        .admit()
        .expect("fixed providers should admit")
}

struct FixedClock;

impl Clock for FixedClock {
    fn unix_seconds(&self) -> intuigram::ProviderResult<i64> {
        Ok(17)
    }
}

struct FixedIds;

impl OperationIdSource for FixedIds {
    fn next_id(&mut self) -> intuigram::ProviderResult<i64> {
        Ok(41)
    }
}
