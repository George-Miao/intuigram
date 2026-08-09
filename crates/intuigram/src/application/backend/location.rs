use intuigram_app::{GeoPointView, PlaceView};

use super::*;

impl Backend {
    pub(super) async fn execute_location(
        &mut self,
        effect: Effect,
        random_id: Option<i64>,
    ) -> Result<AdapterEvent> {
        match effect {
            Effect::SearchPlaces { chat, query, near } => {
                let result = self.client.search_places(chat, query.clone(), near).await;
                Ok(match result {
                    Ok(places) => AdapterEvent::PlaceSearchReady {
                        chat,
                        query,
                        near,
                        places,
                    },
                    Err(source) if source.is_connection_failure() => {
                        return Err(Error::Telegram { source });
                    }
                    Err(error) => AdapterEvent::PlaceSearchFailed {
                        chat,
                        query,
                        near,
                        reason: error.to_string(),
                    },
                })
            }
            Effect::SendStaticLocation {
                chat,
                point,
                local_id,
                reply_to,
                thread_root,
                saved_peer,
            } => {
                let record =
                    LocationRecord::point(chat, local_id, point, reply_to, thread_root, saved_peer);
                self.persist_location(&record, local_id, DeliveryState::Pending)
                    .await?;
                let result = self
                    .client
                    .send_static_location(intuigram_telegram::StaticLocationSend {
                        chat,
                        point,
                        reply_to,
                        thread_root,
                        monoforum_peer: saved_peer,
                        random_id: location_random_id(random_id),
                    })
                    .await;
                self.finish_location(record, result).await
            }
            Effect::SendVenue {
                chat,
                venue,
                local_id,
                reply_to,
                thread_root,
                saved_peer,
            } => {
                let record = LocationRecord::venue(
                    chat,
                    local_id,
                    &venue,
                    reply_to,
                    thread_root,
                    saved_peer,
                );
                self.persist_location(&record, local_id, DeliveryState::Pending)
                    .await?;
                let result = self
                    .client
                    .send_venue(intuigram_telegram::VenueSend {
                        chat,
                        venue,
                        reply_to,
                        thread_root,
                        monoforum_peer: saved_peer,
                        random_id: location_random_id(random_id),
                    })
                    .await;
                self.finish_location(record, result).await
            }
            _ => unreachable!("only location effects are routed here"),
        }
    }

    async fn finish_location(
        &mut self,
        record: LocationRecord,
        result: intuigram_telegram::Result<MessageId>,
    ) -> Result<AdapterEvent> {
        if let Err(source) = &result
            && source.is_connection_failure()
        {
            return Err(Error::Telegram {
                source: result.expect_err("the guarded result is an error"),
            });
        }
        match result {
            Ok(server_id) => {
                let message = location_message(&record, server_id, DeliveryState::Sent);
                self.store
                    .replace_message(
                        record.chat.0,
                        record.local_id.0,
                        encode_stored_message(record.chat, &message),
                    )
                    .context(AccountDatabaseSnafu)?
                    .await
                    .context(AccountDatabaseSnafu)?;
                Ok(AdapterEvent::RichMediaAcknowledged {
                    chat: record.chat,
                    local_id: record.local_id,
                    server_id,
                })
            }
            Err(error) => {
                self.persist_location(&record, record.local_id, DeliveryState::Failed)
                    .await?;
                Ok(AdapterEvent::RichMediaFailed {
                    chat: record.chat,
                    local_id: record.local_id,
                    reason: error.to_string(),
                })
            }
        }
    }

    async fn persist_location(
        &mut self,
        record: &LocationRecord,
        id: MessageId,
        delivery: DeliveryState,
    ) -> Result<()> {
        let message = location_message(record, id, delivery);
        self.store
            .save_messages(vec![encode_stored_message(record.chat, &message)])
            .context(AccountDatabaseSnafu)?
            .await
            .context(AccountDatabaseSnafu)
    }
}

fn location_message(
    record: &LocationRecord,
    id: MessageId,
    delivery: DeliveryState,
) -> MessageView {
    MessageView {
        id,
        sender: "You".to_owned(),
        body: record.body.clone(),
        timestamp: "now".to_owned(),
        direction: MessageDirection::Outgoing,
        delivery,
        reply_to: record.reply_to,
        details: MessageDetails {
            media: Some(record.media.clone()),
            thread_root: record.thread_root,
            saved_peer: record.saved_peer,
            ..MessageDetails::default()
        },
    }
}

struct LocationRecord {
    chat: ChatId,
    local_id: MessageId,
    body: String,
    media: MediaCard,
    reply_to: Option<MessageId>,
    thread_root: Option<MessageId>,
    saved_peer: Option<ChatId>,
}

impl LocationRecord {
    fn point(
        chat: ChatId,
        local_id: MessageId,
        point: GeoPointView,
        reply_to: Option<MessageId>,
        thread_root: Option<MessageId>,
        saved_peer: Option<ChatId>,
    ) -> Self {
        let coordinates = point.coordinates();
        Self {
            chat,
            local_id,
            body: format!("[Location] {coordinates}"),
            media: MediaCard {
                kind: MediaKind::Location,
                title: "Location".to_owned(),
                description: coordinates,
                details: Vec::new(),
                poll: None,
                specialized: None,
                remote_id: None,
            },
            reply_to,
            thread_root,
            saved_peer,
        }
    }

    fn venue(
        chat: ChatId,
        local_id: MessageId,
        venue: &PlaceView,
        reply_to: Option<MessageId>,
        thread_root: Option<MessageId>,
        saved_peer: Option<ChatId>,
    ) -> Self {
        Self {
            chat,
            local_id,
            body: format!("[{}] {}", venue.title, venue.address),
            media: MediaCard {
                kind: MediaKind::Venue,
                title: venue.title.clone(),
                description: venue.address.clone(),
                details: vec![venue.point.coordinates()],
                poll: None,
                specialized: None,
                remote_id: None,
            },
            reply_to,
            thread_root,
            saved_peer,
        }
    }
}

fn location_random_id(random_id: Option<i64>) -> i64 {
    random_id.expect("every queued location send has an idempotency token")
}
