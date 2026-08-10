use intuigram_lib::{AdapterEvent, ChatId, MessageId};
use intuigram_store::{OutboxCompletion, StoredMutation};
use intuigram_telegram::InvocationPolicy;

use super::super::super::{Backend, derived_random_id};
use super::super::model::PreparedCommand;
use super::super::model::mutation::MutationCommand;
use super::{Result, Success, conversion, interaction, telegram};

pub(super) async fn execute(
    backend: &mut Backend,
    prepared: &PreparedCommand,
    command: &MutationCommand,
    media: &[intuigram_store::OutboxMedia],
) -> Result<Success> {
    let chat = ChatId(prepared.destination().chat_id);
    match command {
        MutationCommand::Edit {
            message_id,
            text,
            entities,
            attachments,
        } => {
            let upload = attachments
                .last()
                .map(|attachment| {
                    conversion::media(
                        media,
                        attachment.position,
                        conversion::attachment_kind(attachment.kind),
                    )
                    .map(|upload| {
                        (
                            upload,
                            derived_random_id(random_id(prepared), 0, 0x4544_4954),
                        )
                    })
                })
                .transpose()?;
            telegram(
                backend
                    .client
                    .edit_message_with_policy(
                        intuigram_telegram::MessageEdit {
                            chat,
                            message: MessageId(*message_id),
                            text: text.clone(),
                            entities: conversion::entities(entities),
                            upload,
                        },
                        InvocationPolicy::SurfaceFloodWait,
                    )
                    .await,
            )?;
            acknowledged(Some(AdapterEvent::MessageEditAcknowledged {
                chat,
                message: MessageId(*message_id),
                text: text.clone(),
                entities: conversion::entities(entities),
            }))
        }
        MutationCommand::Delete { message_ids } => {
            let ids = message_ids
                .iter()
                .copied()
                .map(MessageId)
                .collect::<Vec<_>>();
            telegram(
                backend
                    .client
                    .delete_messages_with_policy(
                        chat,
                        ids.clone(),
                        InvocationPolicy::SurfaceFloodWait,
                    )
                    .await,
            )?;
            Ok(Success {
                completion: OutboxCompletion::Mutations(vec![StoredMutation::DeleteMessages {
                    chat_id: Some(chat.0),
                    ids: message_ids.clone(),
                }]),
                event: Some(AdapterEvent::MessagesDeleted {
                    chat: Some(chat),
                    ids,
                }),
                update: None,
            })
        }
        MutationCommand::Forward {
            source_chat_id,
            message_ids,
        } => {
            telegram(
                backend
                    .client
                    .forward_messages_with_policy(
                        ChatId(*source_chat_id),
                        chat,
                        prepared.destination().saved_peer.map(ChatId),
                        message_ids.iter().copied().map(MessageId).collect(),
                        random_id(prepared),
                        InvocationPolicy::SurfaceFloodWait,
                    )
                    .await,
            )?;
            acknowledged(Some(AdapterEvent::OperationCompleted(
                "Messages forwarded".to_owned(),
            )))
        }
        MutationCommand::Reaction {
            message_id,
            reaction,
        } => {
            telegram(
                backend
                    .client
                    .react_message_with_policy(
                        chat,
                        MessageId(*message_id),
                        reaction.clone(),
                        InvocationPolicy::SurfaceFloodWait,
                    )
                    .await,
            )?;
            acknowledged(Some(AdapterEvent::OperationCompleted(
                "Reaction updated".to_owned(),
            )))
        }
        MutationCommand::Pin { message_id, pinned } => {
            let update = telegram(
                backend
                    .client
                    .set_message_pinned_with_policy(
                        chat,
                        MessageId(*message_id),
                        *pinned,
                        InvocationPolicy::SurfaceFloodWait,
                    )
                    .await,
            )?;
            Ok(Success {
                completion: OutboxCompletion::Acknowledged,
                event: None,
                update: Some(update),
            })
        }
        MutationCommand::Vote { .. }
        | MutationCommand::ToggleTodo { .. }
        | MutationCommand::AppendTodo { .. } => interaction::execute(backend, chat, command).await,
    }
}

pub(super) fn acknowledged(event: Option<AdapterEvent>) -> Result<Success> {
    Ok(Success {
        completion: OutboxCompletion::Acknowledged,
        event,
        update: None,
    })
}

fn random_id(command: &PreparedCommand) -> i64 {
    command
        .random_id()
        .expect("validated durable mutations retain their random ID")
}
