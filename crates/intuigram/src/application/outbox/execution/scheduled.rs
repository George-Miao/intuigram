use intuigram_app::{AdapterEvent, ChatId};
use intuigram_telegram::InvocationPolicy;

use super::super::super::Backend;
use super::super::model::PreparedCommand;
use super::super::model::scheduled::{ScheduledCommand, ScheduledDelivery};
use super::mutation::acknowledged;
use super::{Result, Success, telegram};

pub(super) async fn execute(
    backend: &mut Backend,
    prepared: &PreparedCommand,
    command: &ScheduledCommand,
) -> Result<Success> {
    let destination = prepared.destination();
    let chat = ChatId(destination.chat_id);
    match command {
        ScheduledCommand::Create { delivery, text } => telegram(
            backend
                .client
                .schedule_text_with_policy(
                    chat,
                    destination.saved_peer.map(ChatId),
                    text.clone(),
                    scheduled_delivery(*delivery),
                    random_id(prepared),
                    InvocationPolicy::SurfaceFloodWait,
                )
                .await,
        )?,
        ScheduledCommand::Edit { message_id, text } => telegram(
            backend
                .client
                .edit_scheduled_message_with_policy(
                    chat,
                    *message_id,
                    Some(text.clone()),
                    None,
                    InvocationPolicy::SurfaceFloodWait,
                )
                .await,
        )?,
        ScheduledCommand::Reschedule {
            message_id,
            delivery,
        } => telegram(
            backend
                .client
                .edit_scheduled_message_with_policy(
                    chat,
                    *message_id,
                    None,
                    Some(scheduled_delivery(*delivery)),
                    InvocationPolicy::SurfaceFloodWait,
                )
                .await,
        )?,
        ScheduledCommand::Delete { message_id } => telegram(
            backend
                .client
                .delete_scheduled_message_with_policy(
                    chat,
                    *message_id,
                    InvocationPolicy::SurfaceFloodWait,
                )
                .await,
        )?,
        ScheduledCommand::SendNow { message_id } => telegram(
            backend
                .client
                .send_scheduled_now_with_policy(
                    chat,
                    *message_id,
                    InvocationPolicy::SurfaceFloodWait,
                )
                .await,
        )?,
    }
    acknowledged(Some(AdapterEvent::ScheduledOperationAcknowledged {
        chat,
        saved_peer: destination.saved_peer.map(ChatId),
        notice: notice(command).to_owned(),
    }))
}

fn random_id(command: &PreparedCommand) -> i64 {
    command
        .random_id()
        .expect("validated scheduled commands retain their random ID")
}

const fn scheduled_delivery(delivery: ScheduledDelivery) -> intuigram_telegram::ScheduledDelivery {
    match delivery {
        ScheduledDelivery::At(timestamp) => intuigram_telegram::ScheduledDelivery::At(timestamp),
        ScheduledDelivery::WhenOnline => intuigram_telegram::ScheduledDelivery::WhenOnline,
    }
}

const fn notice(command: &ScheduledCommand) -> &'static str {
    match command {
        ScheduledCommand::Create { .. } => "Scheduled Message created",
        ScheduledCommand::Edit { .. } => "Scheduled Message edited",
        ScheduledCommand::Reschedule { .. } => "Scheduled Message rescheduled",
        ScheduledCommand::Delete { .. } => "Scheduled Message deleted",
        ScheduledCommand::SendNow { .. } => "Scheduled Message sent",
    }
}
