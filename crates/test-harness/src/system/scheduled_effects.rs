use intuigram_app::{
    AdapterEvent, ChatId, ScheduledMessageId, ScheduledMessageView, ScheduledRequest,
};

use super::TestSystem;

impl TestSystem {
    pub(super) fn handle_scheduled_load(&mut self, chat: ChatId) {
        self.application
            .handle_adapter(AdapterEvent::ScheduledMessagesReady {
                chat,
                messages: self
                    .scheduled_messages
                    .get(&chat)
                    .cloned()
                    .unwrap_or_default(),
            });
    }

    pub(super) fn handle_scheduled_operation(&mut self, chat: ChatId, request: ScheduledRequest) {
        let notice = match request {
            ScheduledRequest::Create { delivery, text } => {
                self.next_scheduled_id = self.next_scheduled_id.saturating_add(1);
                self.scheduled_messages
                    .entry(chat)
                    .or_default()
                    .push(ScheduledMessageView {
                        id: ScheduledMessageId(self.next_scheduled_id),
                        delivery,
                        summary: text,
                    });
                "Scheduled Message created"
            }
            ScheduledRequest::Edit { message, text } => {
                if let Some(found) = self.scheduled_message_mut(chat, message) {
                    found.summary = text;
                }
                "Scheduled Message edited"
            }
            ScheduledRequest::Reschedule { message, delivery } => {
                if let Some(found) = self.scheduled_message_mut(chat, message) {
                    found.delivery = delivery;
                }
                "Scheduled Message rescheduled"
            }
            ScheduledRequest::Delete { message } => {
                self.remove_scheduled_message(chat, message);
                "Scheduled Message deleted"
            }
            ScheduledRequest::SendNow { message } => {
                self.remove_scheduled_message(chat, message);
                "Scheduled Message sent"
            }
        };
        self.application
            .handle_adapter(AdapterEvent::ScheduledOperationCompleted {
                chat,
                messages: self
                    .scheduled_messages
                    .get(&chat)
                    .cloned()
                    .unwrap_or_default(),
                notice: notice.to_owned(),
            });
    }

    fn scheduled_message_mut(
        &mut self,
        chat: ChatId,
        message: ScheduledMessageId,
    ) -> Option<&mut ScheduledMessageView> {
        self.scheduled_messages
            .entry(chat)
            .or_default()
            .iter_mut()
            .find(|candidate| candidate.id == message)
    }

    fn remove_scheduled_message(&mut self, chat: ChatId, message: ScheduledMessageId) {
        self.scheduled_messages
            .entry(chat)
            .or_default()
            .retain(|candidate| candidate.id != message);
    }
}
