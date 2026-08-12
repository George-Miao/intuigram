use super::{Command, Result};
use crate::{Maintenance, parse_scheduled_maintenance};

impl Command {
    /// Schedules text at an offset time or when the recipient is online.
    pub fn scheduled_create(chat: String, delivery: String, text: String) -> Result<Self> {
        Self::scheduled("scheduled create", "create", [chat, delivery, text])
    }

    /// Lists Telegram-owned Scheduled Messages for a Chat.
    pub fn scheduled_list(chat: String) -> Result<Self> {
        Self::scheduled("scheduled list", "list", [chat])
    }

    /// Replaces a Scheduled Message's text.
    pub fn scheduled_edit(chat: String, message: String, text: String) -> Result<Self> {
        Self::scheduled("scheduled edit", "edit", [chat, message, text])
    }

    /// Changes a Scheduled Message's delivery time.
    pub fn scheduled_reschedule(chat: String, message: String, delivery: String) -> Result<Self> {
        Self::scheduled(
            "scheduled reschedule",
            "reschedule",
            [chat, message, delivery],
        )
    }

    /// Deletes a Scheduled Message after confirmation.
    pub fn scheduled_delete(chat: String, message: String) -> Result<Self> {
        Self::scheduled("scheduled delete", "delete", [chat, message])
    }

    /// Asks Telegram to send a Scheduled Message immediately.
    pub fn scheduled_send_now(chat: String, message: String) -> Result<Self> {
        Self::scheduled("scheduled send-now", "send-now", [chat, message])
    }

    fn scheduled<const N: usize>(label: &str, action: &str, values: [String; N]) -> Result<Self> {
        let command = parse_scheduled_maintenance(&mut values.into_iter(), action, label)?;
        Ok(Self::maintenance(Maintenance::Scheduled(command)))
    }
}
