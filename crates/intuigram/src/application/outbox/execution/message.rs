use intuigram_app::{
    AdapterEvent, ChatId, DeliveryState, MessageDetails, MessageDirection, MessageId, MessageView,
};
use intuigram_store::OutboxCompletion;

use super::super::super::encode_stored_message;
use super::super::model::PreparedCommand;
use super::{Result, Success};

pub(super) fn outgoing(
    command: &PreparedCommand,
    id: MessageId,
    body: String,
    entities: Vec<intuigram_app::TextEntity>,
    reply_to: Option<i64>,
) -> MessageView {
    let destination = command.destination();
    MessageView {
        id,
        sender: "You".to_owned(),
        body,
        timestamp: "now".to_owned(),
        direction: MessageDirection::Outgoing,
        delivery: DeliveryState::Sent,
        reply_to: reply_to.map(MessageId),
        details: MessageDetails {
            entities,
            thread_root: destination.thread_root.map(MessageId),
            saved_peer: destination.saved_peer.map(ChatId),
            ..MessageDetails::default()
        },
    }
}

pub(super) fn success(
    command: &PreparedCommand,
    server_id: MessageId,
    message: MessageView,
) -> Result<Success> {
    let destination = command.destination();
    Ok(Success {
        completion: OutboxCompletion::Message(Box::new(encode_stored_message(
            ChatId(destination.chat_id),
            &message,
        ))),
        event: Some(AdapterEvent::RichMediaAcknowledged {
            chat: ChatId(destination.chat_id),
            local_id: MessageId(
                command
                    .local_message_id()
                    .expect("send completion has an optimistic local Message"),
            ),
            server_id,
        }),
        update: None,
    })
}
