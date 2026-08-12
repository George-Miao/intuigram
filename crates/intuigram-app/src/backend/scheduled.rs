use super::*;

impl Backend {
    pub(super) async fn execute_scheduled(
        &mut self,
        effect: Effect,
        random_id: Option<i64>,
    ) -> Result<AdapterEvent> {
        match effect {
            Effect::LoadScheduledMessages { chat, saved_peer } => {
                let result = self.client.scheduled_messages(chat, saved_peer).await;
                self.scheduled_result(chat, saved_peer, result, None)
            }
            Effect::ScheduledOperation {
                chat,
                saved_peer,
                request,
            } => {
                let notice = scheduled_notice(&request);
                let mutation = self
                    .mutate_scheduled(chat, saved_peer, request, random_id)
                    .await;
                if let Err(source) = mutation {
                    return self.scheduled_result(chat, saved_peer, Err(source), None);
                }
                let result = self.client.scheduled_messages(chat, saved_peer).await;
                Ok(match result {
                    Ok(messages) => AdapterEvent::ScheduledOperationCompleted {
                        chat,
                        saved_peer,
                        messages: messages.into_iter().map(scheduled_message).collect(),
                        notice,
                    },
                    Err(error) => AdapterEvent::ScheduledOperationFailed {
                        chat,
                        saved_peer,
                        reason: format!(
                            "{notice}, but refreshing Scheduled Messages failed: {error}"
                        ),
                    },
                })
            }
            _ => unreachable!("the effect dispatcher only routes Scheduled Message effects"),
        }
    }

    async fn mutate_scheduled(
        &mut self,
        chat: ChatId,
        saved_peer: Option<ChatId>,
        request: ScheduledRequest,
        random_id: Option<i64>,
    ) -> std::result::Result<(), intuigram_telegram::Error> {
        match request {
            ScheduledRequest::Create { delivery, text } => {
                self.client
                    .schedule_text(
                        chat,
                        saved_peer,
                        text,
                        telegram_delivery(delivery),
                        random_id.expect("every queued schedule creation has an idempotency token"),
                    )
                    .await
            }
            ScheduledRequest::Edit { message, text } => {
                self.client
                    .edit_scheduled_message(chat, message.0, Some(text), None)
                    .await
            }
            ScheduledRequest::Reschedule { message, delivery } => {
                self.client
                    .edit_scheduled_message(
                        chat,
                        message.0,
                        None,
                        Some(telegram_delivery(delivery)),
                    )
                    .await
            }
            ScheduledRequest::Delete { message } => {
                self.client.delete_scheduled_message(chat, message.0).await
            }
            ScheduledRequest::SendNow { message } => {
                self.client.send_scheduled_now(chat, message.0).await
            }
        }
    }

    fn scheduled_result(
        &self,
        chat: ChatId,
        saved_peer: Option<ChatId>,
        result: std::result::Result<
            Vec<intuigram_telegram::ScheduledMessage>,
            intuigram_telegram::Error,
        >,
        notice: Option<String>,
    ) -> Result<AdapterEvent> {
        Ok(match result {
            Ok(messages) => {
                let messages = messages.into_iter().map(scheduled_message).collect();
                if let Some(notice) = notice {
                    AdapterEvent::ScheduledOperationCompleted {
                        chat,
                        saved_peer,
                        messages,
                        notice,
                    }
                } else {
                    AdapterEvent::ScheduledMessagesReady {
                        chat,
                        saved_peer,
                        messages,
                    }
                }
            }
            Err(source) if source.is_connection_failure() => {
                return Err(Error::Telegram { source });
            }
            Err(error) => AdapterEvent::ScheduledOperationFailed {
                chat,
                saved_peer,
                reason: error.to_string(),
            },
        })
    }
}

fn telegram_delivery(delivery: ScheduledDeliveryView) -> ScheduledDelivery {
    match delivery {
        ScheduledDeliveryView::At(timestamp) => ScheduledDelivery::At(timestamp),
        ScheduledDeliveryView::WhenOnline => ScheduledDelivery::WhenOnline,
    }
}

fn scheduled_message(message: intuigram_telegram::ScheduledMessage) -> ScheduledMessageView {
    ScheduledMessageView {
        id: ScheduledMessageId(message.id),
        delivery: match message.delivery {
            ScheduledDelivery::At(timestamp) => ScheduledDeliveryView::At(timestamp),
            ScheduledDelivery::WhenOnline => ScheduledDeliveryView::WhenOnline,
        },
        summary: message.summary,
    }
}

fn scheduled_notice(request: &ScheduledRequest) -> String {
    match request {
        ScheduledRequest::Create { .. } => "Scheduled Message created".to_owned(),
        ScheduledRequest::Edit { .. } => "Scheduled Message edited".to_owned(),
        ScheduledRequest::Reschedule { .. } => "Scheduled Message rescheduled".to_owned(),
        ScheduledRequest::Delete { .. } => "Scheduled Message deleted".to_owned(),
        ScheduledRequest::SendNow { .. } => "Scheduled Message sent".to_owned(),
    }
}
