use intuigram_lib::{GeoPointView, PlaceView};

use super::client_location::*;
use super::*;

const POINT: GeoPointView = GeoPointView {
    latitude_microdegrees: 31_230_400,
    longitude_microdegrees: 121_473_700,
};

#[test]
fn inline_request_keeps_exact_peer_query_and_optional_coordinate() {
    let request = inline_results_request(
        tl::types::InputUser {
            user_id: 7,
            access_hash: 8,
        }
        .into(),
        tl::types::InputPeerChat { chat_id: 9 }.into(),
        "coffee".to_owned(),
        Some(POINT),
    );
    assert_eq!(request.query, "coffee");
    assert_eq!(request.offset, "");
    assert!(
        matches!(request.bot, tl::enums::InputUser::User(user) if user.user_id == 7 && user.access_hash == 8)
    );
    assert!(matches!(request.peer, tl::enums::InputPeer::Chat(peer) if peer.chat_id == 9));
    assert!(
        matches!(request.geo_point, Some(tl::enums::InputGeoPoint::Point(point)) if point.lat == 31.2304 && point.long == 121.4737)
    );
}

#[test]
fn inline_results_filter_nonvenues_and_invalid_coordinates() {
    let venue = venue_message();
    let results = normalize_place_results(vec![
        plain_result("a", venue.clone()),
        plain_result("text", text_message()),
        plain_result("invalid", invalid_venue_message()),
        tl::types::BotInlineMediaResult {
            id: "b".to_owned(),
            r#type: "venue".to_owned(),
            photo: None,
            document: None,
            title: None,
            description: None,
            send_message: venue,
        }
        .into(),
    ]);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].point, POINT);
    assert_eq!(results[0].title, "Coffee Lab");
}

#[test]
fn place_results_are_bounded_for_the_composer() {
    let results = (0..51)
        .map(|index| plain_result(&index.to_string(), venue_message()))
        .collect();
    assert_eq!(normalize_place_results(results).len(), 50);
}

#[test]
fn resolved_provider_must_be_the_exact_bot_with_an_access_hash() {
    let mut unrelated = user(8, true, Some(80));
    let missing_hash = user(7, true, None);
    assert!(resolved_venue_bot(7, &[unrelated.clone().into(), missing_hash.into()]).is_none());
    unrelated.id = 7;
    assert!(matches!(
        resolved_venue_bot(7, &[unrelated.into()]),
        Some(tl::enums::InputUser::User(bot)) if bot.user_id == 7 && bot.access_hash == 80
    ));
    assert!(resolved_venue_bot(7, &[user(7, false, Some(80)).into()]).is_none());
}

#[test]
fn send_media_builders_preserve_exact_normalized_payload() {
    assert!(
        matches!(location_media(POINT), tl::enums::InputMedia::GeoPoint(media)
        if matches!(&media.geo_point, tl::enums::InputGeoPoint::Point(point) if point.lat == 31.2304 && point.long == 121.4737))
    );
    assert!(
        matches!(venue_media(place()), tl::enums::InputMedia::Venue(media)
        if media.title == "Coffee Lab" && media.provider == "foursquare" && media.venue_id == "venue-7")
    );
}

fn plain_result(id: &str, message: tl::enums::BotInlineMessage) -> tl::enums::BotInlineResult {
    tl::types::BotInlineResult {
        id: id.to_owned(),
        r#type: "venue".to_owned(),
        title: None,
        description: None,
        url: None,
        thumb: None,
        content: None,
        send_message: message,
    }
    .into()
}

fn venue_message() -> tl::enums::BotInlineMessage {
    tl::types::BotInlineMessageMediaVenue {
        geo: tl::types::GeoPoint {
            long: 121.4737,
            lat: 31.2304,
            access_hash: 0,
            accuracy_radius: None,
        }
        .into(),
        title: "Coffee Lab".to_owned(),
        address: "1 Test Street".to_owned(),
        provider: "foursquare".to_owned(),
        venue_id: "venue-7".to_owned(),
        venue_type: "coffee".to_owned(),
        reply_markup: None,
    }
    .into()
}

fn invalid_venue_message() -> tl::enums::BotInlineMessage {
    let tl::enums::BotInlineMessage::MediaVenue(mut venue) = venue_message() else {
        unreachable!("the fixture is a venue")
    };
    venue.geo = tl::types::GeoPoint {
        long: 121.0,
        lat: 91.0,
        access_hash: 0,
        accuracy_radius: None,
    }
    .into();
    venue.into()
}

fn text_message() -> tl::enums::BotInlineMessage {
    tl::types::BotInlineMessageText {
        no_webpage: true,
        invert_media: false,
        message: "not a venue".to_owned(),
        entities: None,
        reply_markup: None,
    }
    .into()
}

fn place() -> PlaceView {
    PlaceView {
        point: POINT,
        title: "Coffee Lab".to_owned(),
        address: "1 Test Street".to_owned(),
        provider: "foursquare".to_owned(),
        venue_id: "venue-7".to_owned(),
        venue_type: "coffee".to_owned(),
    }
}

fn user(id: i64, bot: bool, access_hash: Option<i64>) -> tl::types::User {
    let mut user = crate::tests::media_fixtures::user(id, false, bot);
    user.access_hash = access_hash;
    user
}
