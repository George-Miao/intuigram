use super::*;

impl Backend {
    pub(in crate::application) async fn execute_message_send(
        &mut self,
        effect: Effect,
        random_id: Option<i64>,
    ) -> Result<Option<AdapterEvent>> {
        let Effect::SendMessage {
            chat,
            text,
            entities,
            link_preview,
            reply_to,
            thread_root,
            saved_peer,
            attachments,
            local_id,
        } = effect
        else {
            unreachable!("message send routing only passes SendMessage effects");
        };
        self.persist_outgoing(OutgoingRecord {
            chat,
            local_id,
            text: &text,
            entities: &entities,
            reply_to,
            thread_root,
            saved_peer,
            delivery: DeliveryState::Pending,
        })
        .await?;
        self.save_draft(chat, thread_root, saved_peer, String::new(), None)
            .await?;
        let result = self
            .send_message(MessageSend {
                chat,
                text: text.clone(),
                entities: entities.clone(),
                link_preview,
                reply_to,
                thread_root,
                saved_peer,
                attachment_ids: attachments,
                random_id: random_id.expect("every queued send has an idempotency token"),
            })
            .await;
        let result = match result {
            Err(Error::Telegram { source }) if source.is_connection_failure() => {
                return Err(Error::Telegram { source });
            }
            result => result,
        };
        match &result {
            Ok(message) => {
                self.acknowledge_outgoing(
                    OutgoingRecord {
                        chat,
                        local_id,
                        text: &text,
                        entities: &entities,
                        reply_to,
                        thread_root,
                        saved_peer,
                        delivery: DeliveryState::Sent,
                    },
                    message.id,
                )
                .await?;
            }
            Err(_) => {
                self.persist_outgoing(OutgoingRecord {
                    chat,
                    local_id,
                    text: &text,
                    entities: &entities,
                    reply_to,
                    thread_root,
                    saved_peer,
                    delivery: DeliveryState::Failed,
                })
                .await?;
            }
        }
        Ok(Some(match result {
            Ok(message) => successful_send_event(chat, message),
            Err(error) => AdapterEvent::MessageFailed {
                chat,
                local_id,
                thread_root,
                saved_peer,
                text,
                reason: error.to_string(),
            },
        }))
    }
}

fn successful_send_event(chat: ChatId, message: MessageView) -> AdapterEvent {
    AdapterEvent::MessageAdded {
        chat,
        message: Box::new(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_send_keeps_the_server_message_identity() {
        let event = successful_send_event(
            ChatId(10),
            outgoing_message(
                &OutgoingRecord {
                    chat: ChatId(10),
                    local_id: MessageId(-1),
                    text: "hello",
                    entities: &[],
                    reply_to: None,
                    thread_root: None,
                    saved_peer: None,
                    delivery: DeliveryState::Sent,
                },
                MessageId(41),
                DeliveryState::Sent,
            ),
        );

        assert!(matches!(
            event,
            AdapterEvent::MessageAdded { chat: ChatId(10), message }
                if message.id == MessageId(41)
        ));
    }
}
