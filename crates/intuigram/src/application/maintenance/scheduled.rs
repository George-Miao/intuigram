use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use super::*;

pub(crate) async fn run_scheduled_maintenance(
    config: &Config,
    config_directory: &std::path::Path,
    account: AccountId,
    command: ScheduledMaintenance,
) -> Result<()> {
    let mut client = connect_account(config, config_directory, account).await?;
    match command {
        ScheduledMaintenance::Create {
            chat,
            delivery,
            text,
        } => {
            client
                .schedule_text(chat, None, text, delivery, operation_id()?)
                .await
                .context(TelegramSnafu)?;
            println!("Scheduled a Message for Chat {}.", chat.0);
        }
        ScheduledMaintenance::List { chat } => {
            for message in client
                .scheduled_messages(chat, None)
                .await
                .context(TelegramSnafu)?
            {
                println!(
                    "{}\t{}\t{}",
                    message.id,
                    format_delivery(message.delivery),
                    message.summary.replace(['\r', '\n'], " ")
                );
            }
        }
        ScheduledMaintenance::Edit {
            chat,
            message,
            text,
        } => {
            client
                .edit_scheduled_message(chat, message, Some(text), None)
                .await
                .context(TelegramSnafu)?;
            println!("Edited Scheduled Message {message} in Chat {}.", chat.0);
        }
        ScheduledMaintenance::Reschedule {
            chat,
            message,
            delivery,
        } => {
            client
                .edit_scheduled_message(chat, message, None, Some(delivery))
                .await
                .context(TelegramSnafu)?;
            println!("Rescheduled Message {message} in Chat {}.", chat.0);
        }
        ScheduledMaintenance::Delete { chat, message } => {
            let confirmation = prompt(
                &format!("Type DELETE SCHEDULED {message} to continue"),
                "Scheduled Message deletion confirmation",
            )?;
            if confirmation != format!("DELETE SCHEDULED {message}") {
                println!("Scheduled Message was not changed.");
                return Ok(());
            }
            client
                .delete_scheduled_message(chat, message)
                .await
                .context(TelegramSnafu)?;
            println!("Deleted Scheduled Message {message} from Chat {}.", chat.0);
        }
        ScheduledMaintenance::SendNow { chat, message } => {
            client
                .send_scheduled_now(chat, message)
                .await
                .context(TelegramSnafu)?;
            println!(
                "Requested immediate delivery of Message {message} in Chat {}.",
                chat.0
            );
        }
    }
    Ok(())
}

fn operation_id() -> Result<i64> {
    let mut bytes = [0_u8; 8];
    getrandom::fill(&mut bytes).context(OperationIdSnafu)?;
    Ok(i64::from_le_bytes(bytes))
}

fn format_timestamp(timestamp: i32) -> String {
    OffsetDateTime::from_unix_timestamp(i64::from(timestamp))
        .ok()
        .and_then(|date| date.format(&Rfc3339).ok())
        .unwrap_or_else(|| timestamp.to_string())
}

fn format_delivery(delivery: ScheduledDelivery) -> String {
    match delivery {
        ScheduledDelivery::At(timestamp) => format_timestamp(timestamp),
        ScheduledDelivery::WhenOnline => "when-online".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_list_uses_stable_utc_timestamps() {
        assert_eq!(format_timestamp(0), "1970-01-01T00:00:00Z");
    }
}
