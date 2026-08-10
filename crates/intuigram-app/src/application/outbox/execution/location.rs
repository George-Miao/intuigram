use intuigram_lib::{ChatId, MediaCard, MediaKind, MessageId, PlaceView};
use intuigram_telegram::InvocationPolicy;

use super::super::super::Backend;
use super::super::model::PreparedCommand;
use super::super::model::send::{MessageSend, Venue};
use super::super::model::shared::GeoPoint;
use super::message::{outgoing, success};
use super::{Result, telegram};

pub(super) async fn location(
    backend: &mut Backend,
    command: &PreparedCommand,
    send: &MessageSend<GeoPoint>,
) -> Result<super::Success> {
    let point = point(send.content);
    let destination = command.destination();
    let server_id = telegram(
        backend
            .client
            .send_static_location_with_policy(
                intuigram_telegram::StaticLocationSend {
                    chat: ChatId(destination.chat_id),
                    point,
                    reply_to: send.reply_to.map(MessageId),
                    thread_root: destination.thread_root.map(MessageId),
                    monoforum_peer: destination.saved_peer.map(ChatId),
                    random_id: random_id(command),
                },
                InvocationPolicy::SurfaceFloodWait,
            )
            .await,
    )?;
    let coordinates = point.coordinates();
    let mut message = outgoing(
        command,
        server_id,
        format!("[Location] {coordinates}"),
        Vec::new(),
        send.reply_to,
    );
    message.details.media = Some(location_card(
        "Location",
        &coordinates,
        Vec::new(),
        MediaKind::Location,
    ));
    success(command, server_id, message)
}

pub(super) async fn venue(
    backend: &mut Backend,
    command: &PreparedCommand,
    send: &MessageSend<Venue>,
) -> Result<super::Success> {
    let content = &send.content;
    let venue = PlaceView {
        point: point(content.point),
        title: content.title.clone(),
        address: content.address.clone(),
        provider: content.provider.clone(),
        venue_id: content.venue_id.clone(),
        venue_type: content.venue_type.clone(),
    };
    let destination = command.destination();
    let server_id = telegram(
        backend
            .client
            .send_venue_with_policy(
                intuigram_telegram::VenueSend {
                    chat: ChatId(destination.chat_id),
                    venue: venue.clone(),
                    reply_to: send.reply_to.map(MessageId),
                    thread_root: destination.thread_root.map(MessageId),
                    monoforum_peer: destination.saved_peer.map(ChatId),
                    random_id: random_id(command),
                },
                InvocationPolicy::SurfaceFloodWait,
            )
            .await,
    )?;
    let mut message = outgoing(
        command,
        server_id,
        format!("[{}] {}", venue.title, venue.address),
        Vec::new(),
        send.reply_to,
    );
    message.details.media = Some(location_card(
        &venue.title,
        &venue.address,
        vec![venue.point.coordinates()],
        MediaKind::Venue,
    ));
    success(command, server_id, message)
}

fn point(point: GeoPoint) -> intuigram_lib::GeoPointView {
    intuigram_lib::GeoPointView {
        latitude_microdegrees: point.latitude_microdegrees,
        longitude_microdegrees: point.longitude_microdegrees,
    }
}

fn location_card(
    title: &str,
    description: &str,
    details: Vec<String>,
    kind: MediaKind,
) -> MediaCard {
    MediaCard {
        kind,
        title: title.to_owned(),
        description: description.to_owned(),
        details,
        poll: None,
        specialized: None,
        remote_id: None,
    }
}

fn random_id(command: &PreparedCommand) -> i64 {
    command
        .random_id()
        .expect("validated durable location sends retain their random ID")
}
