use intuigram_lib::{AdapterEvent, ChatId, MessageId};
use intuigram_telegram::InvocationPolicy;

use super::super::super::Backend;
use super::super::model::mutation::MutationCommand;
use super::mutation::acknowledged;
use super::{InvalidSnafu, Result, Success, telegram};

pub(super) async fn execute(
    backend: &mut Backend,
    chat: ChatId,
    command: &MutationCommand,
) -> Result<Success> {
    let (message, media) = match command {
        MutationCommand::Vote {
            message_id,
            options,
        } => (
            *message_id,
            telegram(
                backend
                    .client
                    .vote_poll_with_policy(
                        chat,
                        MessageId(*message_id),
                        options.iter().map(|option| *option as usize).collect(),
                        InvocationPolicy::SurfaceFloodWait,
                    )
                    .await,
            )?,
        ),
        MutationCommand::ToggleTodo {
            message_id,
            item_id,
            completed,
        } => (
            *message_id,
            telegram(
                backend
                    .client
                    .toggle_todo_item_with_policy(
                        chat,
                        MessageId(*message_id),
                        *item_id,
                        *completed,
                        InvocationPolicy::SurfaceFloodWait,
                    )
                    .await,
            )?,
        ),
        MutationCommand::AppendTodo { message_id, title } => (
            *message_id,
            telegram(
                backend
                    .client
                    .append_todo_item_with_policy(
                        chat,
                        MessageId(*message_id),
                        title.clone(),
                        InvocationPolicy::SurfaceFloodWait,
                    )
                    .await,
            )?,
        ),
        _ => {
            return InvalidSnafu {
                reason: "non-interactive mutation reached interactive executor",
            }
            .fail();
        }
    };
    acknowledged(Some(AdapterEvent::MessageMediaUpdated {
        chat,
        message: MessageId(message),
        media,
    }))
}
