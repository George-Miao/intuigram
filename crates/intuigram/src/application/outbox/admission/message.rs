use intuigram::OperationStamp;
use intuigram_app::{
    DeliveryState, Effect, MediaCard, MediaKind, MessageDetails, MessageDirection, MessageId,
    MessageView, PollOptionView, PollView,
};
use intuigram_store::{
    OutboxAdmission, OutboxExpiry, OutboxMedia, OutboxOperation, OutboxPayload, OutboxPayloadV1,
};
use snafu::ResultExt;

use super::super::super::encode_stored_message;
use super::super::codec;
use super::super::model::send::{MessageSend, Poll, TextMessage};
use super::super::model::{Command, Destination, PreparedCommand};
use super::input::PreparedInputs;
use super::{Result, conversion};

pub(super) fn prepare(
    effect: &Effect,
    stamp: OperationStamp,
    mut inputs: PreparedInputs,
) -> Result<OutboxAdmission> {
    let (destination, local_id, reply_to, command, media, message) = match effect {
        Effect::SendMessage {
            chat,
            text,
            entities,
            link_preview,
            reply_to,
            thread_root,
            saved_peer,
            attachments,
            local_id,
        } => {
            let (prepared, media) = inputs.attachments(attachments)?;
            let content = TextMessage {
                text: text.clone(),
                entities: conversion::entities(entities)?,
                link_preview: *link_preview,
                attachments: prepared,
            };
            let message = outgoing(*local_id, text.clone(), entities.clone(), *reply_to);
            (
                destination(*chat, *thread_root, *saved_peer),
                *local_id,
                *reply_to,
                Command::Text(MessageSend::new(
                    local_id.0,
                    reply_to.map(|id| id.0),
                    content,
                )),
                media,
                message,
            )
        }
        Effect::SendPoll {
            chat,
            question,
            options,
            reply_to,
            thread_root,
            saved_peer,
            local_id,
        } => (
            destination(*chat, *thread_root, *saved_peer),
            *local_id,
            *reply_to,
            Command::Poll(MessageSend::new(
                local_id.0,
                reply_to.map(|id| id.0),
                Poll {
                    question: question.clone(),
                    options: options.clone(),
                },
            )),
            Vec::new(),
            poll_message(*local_id, question, options, *reply_to),
        ),
        _ => return super::media::prepare(effect, stamp, inputs),
    };
    finish(
        destination,
        local_id,
        reply_to,
        command,
        media,
        message,
        stamp,
    )
}

pub(super) fn finish(
    destination: Destination,
    local_id: MessageId,
    _reply_to: Option<MessageId>,
    command: Command,
    media: Vec<OutboxMedia>,
    mut message: MessageView,
    stamp: OperationStamp,
) -> Result<OutboxAdmission> {
    message.details.thread_root = destination.thread_root.map(MessageId);
    message.details.saved_peer = destination.saved_peer.map(intuigram_app::ChatId);
    let command = PreparedCommand::new(destination, Some(stamp.random_id()), command);
    Ok(OutboxAdmission {
        operation: OutboxOperation::Send,
        payload: OutboxPayload::V1(OutboxPayloadV1 {
            chat_id: destination.chat_id,
            thread_root: destination.thread_root,
            saved_peer: destination.saved_peer,
            local_message_id: Some(local_id.0),
            random_id: stamp.random_id(),
            content: codec::encode(&command).context(super::EncodeSnafu)?,
        }),
        media,
        optimistic_message: Some(encode_stored_message(
            intuigram_app::ChatId(destination.chat_id),
            &message,
        )),
        consume_draft: true,
        admitted_at: stamp.observed_at(),
        expiry: OutboxExpiry::Never,
    })
}

pub(super) fn outgoing(
    id: MessageId,
    body: String,
    entities: Vec<intuigram_app::TextEntity>,
    reply_to: Option<MessageId>,
) -> MessageView {
    MessageView {
        id,
        sender: "You".to_owned(),
        body,
        timestamp: "now".to_owned(),
        direction: MessageDirection::Outgoing,
        delivery: DeliveryState::Pending,
        reply_to,
        details: MessageDetails {
            entities,
            ..MessageDetails::default()
        },
    }
}

fn poll_message(
    id: MessageId,
    question: &str,
    options: &[String],
    reply_to: Option<MessageId>,
) -> MessageView {
    let mut message = outgoing(id, String::new(), Vec::new(), reply_to);
    message.details.media = Some(MediaCard {
        kind: MediaKind::Poll,
        title: "Poll".to_owned(),
        description: question.to_owned(),
        details: Vec::new(),
        poll: Some(PollView {
            quiz: false,
            multiple_choice: false,
            closed: false,
            total_voters: Some(0),
            options: options
                .iter()
                .map(|text| PollOptionView {
                    text: text.clone(),
                    voters: Some(0),
                    chosen: false,
                    correct: false,
                })
                .collect(),
            solution: None,
        }),
        specialized: None,
        remote_id: None,
    });
    message
}

pub(super) fn destination(
    chat: intuigram_app::ChatId,
    thread: Option<MessageId>,
    saved_peer: Option<intuigram_app::ChatId>,
) -> Destination {
    Destination {
        chat_id: chat.0,
        thread_root: thread.map(|id| id.0),
        saved_peer: saved_peer.map(|id| id.0),
    }
}
