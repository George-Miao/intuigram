use intuigram::OperationStamp;
use intuigram_app::Effect;
use intuigram_store::{
    OutboxAdmission, OutboxExpiry, OutboxOperation, OutboxPayload, OutboxPayloadV1,
};
use snafu::ResultExt;

use super::super::codec;
use super::super::model::mutation::MutationCommand;
use super::super::model::{Command, Destination, PreparedCommand};
use super::input::PreparedInputs;
use super::{Error, Result, conversion};

pub(super) fn prepare(
    effect: &Effect,
    stamp: OperationStamp,
    mut inputs: PreparedInputs,
) -> Result<OutboxAdmission> {
    let (destination, operation, command, media) = match effect {
        Effect::ScheduledOperation {
            chat,
            saved_peer,
            request,
        } => {
            let (operation, command) = super::scheduled::prepare(request);
            (
                Destination {
                    chat_id: chat.0,
                    thread_root: None,
                    saved_peer: saved_peer.map(|peer| peer.0),
                },
                operation,
                Command::Scheduled(command),
                Vec::new(),
            )
        }
        Effect::EditMessage {
            chat,
            message,
            attachments,
            ..
        } => {
            let (attachments, media) = inputs.attachments(attachments)?;
            (
                message_destination(*chat, message),
                OutboxOperation::Mutation,
                Command::Mutation(MutationCommand::Edit {
                    message_id: message.id.0,
                    text: message.body.clone(),
                    entities: conversion::entities(&message.details.entities)?,
                    attachments,
                }),
                media,
            )
        }
        Effect::DeleteMessages { chat, messages } => (
            root(*chat, None),
            OutboxOperation::Mutation,
            Command::Mutation(MutationCommand::Delete {
                message_ids: messages.iter().map(|message| message.0).collect(),
            }),
            Vec::new(),
        ),
        Effect::ForwardMessages {
            source,
            destination,
            destination_saved_peer,
            messages,
        } => (
            root(*destination, *destination_saved_peer),
            OutboxOperation::Send,
            Command::Mutation(MutationCommand::Forward {
                source_chat_id: source.0,
                message_ids: messages.iter().map(|message| message.0).collect(),
            }),
            Vec::new(),
        ),
        Effect::ReactMessage {
            chat,
            message,
            reaction,
        } => (
            message_destination(*chat, message),
            OutboxOperation::Mutation,
            Command::Mutation(MutationCommand::Reaction {
                message_id: message.id.0,
                reaction: reaction.clone(),
            }),
            Vec::new(),
        ),
        Effect::SetMessagePinned {
            chat,
            message,
            pinned,
        } => (
            root(*chat, None),
            OutboxOperation::Mutation,
            Command::Mutation(MutationCommand::Pin {
                message_id: message.0,
                pinned: *pinned,
            }),
            Vec::new(),
        ),
        Effect::VotePoll {
            chat,
            message,
            options,
        } => (
            message_destination(*chat, message),
            OutboxOperation::Mutation,
            Command::Mutation(MutationCommand::Vote {
                message_id: message.id.0,
                options: options
                    .iter()
                    .map(|option| u32::try_from(*option).map_err(|_| Error::NumericOverflow))
                    .collect::<Result<Vec<_>>>()?,
            }),
            Vec::new(),
        ),
        Effect::ToggleTodoItem {
            chat,
            message,
            item,
            completed,
        } => (
            message_destination(*chat, message),
            OutboxOperation::Mutation,
            Command::Mutation(MutationCommand::ToggleTodo {
                message_id: message.id.0,
                item_id: *item,
                completed: *completed,
            }),
            Vec::new(),
        ),
        Effect::AppendTodoItem {
            chat,
            message,
            title,
        } => (
            message_destination(*chat, message),
            OutboxOperation::Mutation,
            Command::Mutation(MutationCommand::AppendTodo {
                message_id: message.id.0,
                title: title.clone(),
            }),
            Vec::new(),
        ),
        _ => {
            return Err(Error::Incomplete {
                reason: "mutation has no admission mapping",
            });
        }
    };
    let command = PreparedCommand::new(destination, Some(stamp.random_id()), command);
    Ok(OutboxAdmission {
        operation,
        payload: OutboxPayload::V1(OutboxPayloadV1 {
            chat_id: destination.chat_id,
            thread_root: destination.thread_root,
            saved_peer: destination.saved_peer,
            local_message_id: None,
            random_id: stamp.random_id(),
            content: codec::encode(&command).context(super::EncodeSnafu)?,
        }),
        media,
        optimistic_message: None,
        consume_draft: false,
        admitted_at: stamp.observed_at(),
        expiry: OutboxExpiry::Never,
    })
}

fn message_destination(
    chat: intuigram_app::ChatId,
    message: &intuigram_app::MessageView,
) -> Destination {
    Destination {
        chat_id: chat.0,
        thread_root: message.details.thread_root.map(|message| message.0),
        saved_peer: message.details.saved_peer.map(|peer| peer.0),
    }
}

const fn root(
    chat: intuigram_app::ChatId,
    saved_peer: Option<intuigram_app::ChatId>,
) -> Destination {
    Destination {
        chat_id: chat.0,
        thread_root: None,
        saved_peer: match saved_peer {
            Some(peer) => Some(peer.0),
            None => None,
        },
    }
}
