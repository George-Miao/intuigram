use super::*;

const SCHEDULE_WHEN_ONLINE: i32 = 0x7fff_fffe;

/// Server-owned delivery trigger for a Scheduled Message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScheduledDelivery {
    /// Deliver at this UTC Unix timestamp.
    At(i32),

    /// Deliver the next time the recipient is online, where Telegram permits.
    WhenOnline,
}

impl ScheduledDelivery {
    const fn telegram_date(self) -> i32 {
        match self {
            Self::At(timestamp) => timestamp,
            Self::WhenOnline => SCHEDULE_WHEN_ONLINE,
        }
    }

    const fn from_telegram_date(timestamp: i32) -> Self {
        if timestamp == SCHEDULE_WHEN_ONLINE {
            Self::WhenOnline
        } else {
            Self::At(timestamp)
        }
    }
}

/// One server-owned Scheduled Message summary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduledMessage {
    /// Telegram Message identifier used by edit/delete/send-now operations.
    pub id: i32,
    /// UTC Unix timestamp at which Telegram plans to deliver it.
    pub delivery: ScheduledDelivery,
    /// Text or a stable media fallback.
    pub summary: String,
}

impl Client {
    /// Schedules a plain text Message for server-side delivery.
    pub async fn schedule_text(
        &mut self,
        chat: ChatId,
        text: String,
        delivery: ScheduledDelivery,
        random_id: i64,
    ) -> Result<()> {
        self.send_text(TextSend {
            chat,
            text,
            entities: Vec::new(),
            link_preview: true,
            reply_to: None,
            thread_root: None,
            random_id,
            schedule_date: Some(delivery.telegram_date()),
        })
        .await
        .map(|_| ())
    }

    /// Lists the Scheduled Messages Telegram currently owns for one Chat.
    pub async fn scheduled_messages(&mut self, chat: ChatId) -> Result<Vec<ScheduledMessage>> {
        let peer = self.peers.resolve(chat)?;
        let response = self
            .connection
            .invoke(&tl::functions::messages::GetScheduledHistory { peer, hash: 0 })
            .await
            .context(InvokeSnafu)?;
        Ok(message_list(response)
            .into_iter()
            .filter_map(scheduled_summary)
            .collect())
    }

    /// Changes the text and/or server delivery time of a Scheduled Message.
    pub async fn edit_scheduled_message(
        &mut self,
        chat: ChatId,
        message_id: i32,
        text: Option<String>,
        delivery: Option<ScheduledDelivery>,
    ) -> Result<()> {
        let peer = self.peers.resolve(chat)?;
        self.connection
            .invoke(&tl::functions::messages::EditMessage {
                no_webpage: false,
                invert_media: false,
                peer,
                id: message_id,
                message: text,
                media: None,
                reply_markup: None,
                entities: None,
                schedule_date: delivery.map(ScheduledDelivery::telegram_date),
                schedule_repeat_period: None,
                quick_reply_shortcut_id: None,
                rich_message: None,
            })
            .await
            .context(InvokeSnafu)?;
        Ok(())
    }

    /// Deletes a Scheduled Message without sending it.
    pub async fn delete_scheduled_message(&mut self, chat: ChatId, message_id: i32) -> Result<()> {
        let peer = self.peers.resolve(chat)?;
        self.connection
            .invoke(&tl::functions::messages::DeleteScheduledMessages {
                peer,
                id: vec![message_id],
            })
            .await
            .context(InvokeSnafu)?;
        Ok(())
    }

    /// Requests immediate server delivery of a Scheduled Message.
    pub async fn send_scheduled_now(&mut self, chat: ChatId, message_id: i32) -> Result<()> {
        let peer = self.peers.resolve(chat)?;
        self.connection
            .invoke(&tl::functions::messages::SendScheduledMessages {
                peer,
                id: vec![message_id],
            })
            .await
            .context(InvokeSnafu)?;
        Ok(())
    }
}

fn message_list(response: tl::enums::messages::Messages) -> Vec<tl::enums::Message> {
    match response {
        tl::enums::messages::Messages::Messages(messages) => messages.messages,
        tl::enums::messages::Messages::Slice(messages) => messages.messages,
        tl::enums::messages::Messages::ChannelMessages(messages) => messages.messages,
        tl::enums::messages::Messages::NotModified(_) => Vec::new(),
    }
}

fn scheduled_summary(message: tl::enums::Message) -> Option<ScheduledMessage> {
    match message {
        tl::enums::Message::Message(message) => Some(ScheduledMessage {
            id: message.id,
            delivery: ScheduledDelivery::from_telegram_date(message.date),
            summary: if message.message.is_empty() && message.media.is_some() {
                "[media]".to_owned()
            } else {
                message.message
            },
        }),
        tl::enums::Message::Service(message) => Some(ScheduledMessage {
            id: message.id,
            delivery: ScheduledDelivery::from_telegram_date(message.date),
            summary: "[service message]".to_owned(),
        }),
        tl::enums::Message::Empty(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{SCHEDULE_WHEN_ONLINE, ScheduledDelivery};

    #[test]
    fn when_online_uses_telegrams_reserved_schedule_date() {
        assert_eq!(
            ScheduledDelivery::WhenOnline.telegram_date(),
            SCHEDULE_WHEN_ONLINE
        );
        assert_eq!(
            ScheduledDelivery::from_telegram_date(SCHEDULE_WHEN_ONLINE),
            ScheduledDelivery::WhenOnline
        );
    }
}
