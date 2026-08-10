use super::*;
use crate::{GeoPointView, LocationParseError, MediaKind, PlaceView, parse_geo_point};

#[test]
fn explicit_coordinates_and_direct_map_urls_parse_without_floating_point() {
    let expected = GeoPointView {
        latitude_microdegrees: 31_230_400,
        longitude_microdegrees: 121_473_700,
    };
    for input in [
        "31.2304, 121.4737",
        "geo:31.230400,121.473700?z=16",
        "https://maps.apple.com/?ll=31.230400%2C121.473700",
        "https://maps.apple.com/place?coordinate=31.230400%2C121.473700",
        "https://www.google.com/maps?q=31.230400%2C121.473700",
        "https://www.google.com/maps?query=31.230400%2C121.473700",
        "https://maps.google.com/maps/@31.230400,121.473700,16z",
        "https://www.openstreetmap.org/?mlat=31.230400&mlon=121.473700",
        "https://www.openstreetmap.org/#map=16/31.230400/121.473700",
    ] {
        assert_eq!(parse_geo_point(input), Ok(expected), "{input}");
    }
    assert_eq!(expected.coordinates(), "31.230400, 121.473700");
    assert_eq!(
        parse_geo_point("-0.000000,+0"),
        Ok(GeoPointView {
            latitude_microdegrees: 0,
            longitude_microdegrees: 0,
        })
    );
}

#[test]
fn redirectors_arbitrary_hosts_and_invalid_coordinates_are_rejected() {
    for input in [
        "https://maps.app.goo.gl/abc",
        "https://goo.gl/maps/abc",
        "https://example.com/?q=31.2,121.4",
        "http://maps.apple.com/?ll=31.2,121.4",
        "https://maps.apple.com/?ll=31.2,121.4&ll=32,122",
        "https://maps.apple.com/?ll=31.2,121.4&coordinate=32,122",
        "https://www.google.com/maps?q=31.2,121.4&query=32,122",
        "https://www.openstreetmap.org/?mlat=31.2#map=16/31.2/121.4",
    ] {
        assert_eq!(parse_geo_point(input), Err(LocationParseError::Unsupported));
    }
    assert_eq!(
        parse_geo_point("91, 121"),
        Err(LocationParseError::OutOfRange)
    );
    assert_eq!(
        parse_geo_point("31.1234567,121"),
        Err(LocationParseError::InvalidCoordinate)
    );
    for input in ["-+1,2", "+-1,2", "1.,2", "--1,2"] {
        assert_eq!(
            parse_geo_point(input),
            Err(LocationParseError::InvalidCoordinate),
            "{input}"
        );
    }
}

#[test]
fn coordinate_submission_preserves_reply_and_thread_context() {
    let mut app = opened_app();
    apply(
        &mut app,
        Input::Intent(Intent::Action(Action::OpenRichMedia)),
    );
    for _ in 0..7 {
        apply(&mut app, Input::Intent(Intent::Action(Action::MoveDown)));
    }
    apply(
        &mut app,
        Input::Intent(Intent::Action(Action::ChooseRichMedia)),
    );
    apply(
        &mut app,
        Input::Intent(Intent::Insert("31.2304,121.4737".to_owned())),
    );

    let update = app.transition(Input::Intent(Intent::Action(Action::ChooseRichMedia)));

    assert!(matches!(
        update.effect,
        Some(Effect::SendStaticLocation {
            chat: ChatId(10),
            point: GeoPointView {
                latitude_microdegrees: 31_230_400,
                longitude_microdegrees: 121_473_700,
            },
            reply_to: None,
            thread_root: None,
            saved_peer: None,
            ..
        })
    ));
    assert_eq!(
        update
            .view
            .messages
            .last()
            .map(|message| message.body.as_str()),
        Some("[Location] 31.230400, 121.473700")
    );
    assert_eq!(
        update
            .view
            .messages
            .last()
            .and_then(|message| message.details.media.as_ref())
            .map(|media| (media.kind, media.description.as_str())),
        Some((MediaKind::Location, "31.230400, 121.473700"))
    );
}

#[test]
fn place_results_are_correlated_and_sent_as_normalized_venues() {
    let mut app = opened_app();
    apply(
        &mut app,
        Input::Intent(Intent::Action(Action::OpenRichMedia)),
    );
    for _ in 0..8 {
        apply(&mut app, Input::Intent(Intent::Action(Action::MoveDown)));
    }
    apply(
        &mut app,
        Input::Intent(Intent::Action(Action::ChooseRichMedia)),
    );
    apply(&mut app, Input::Intent(Intent::Insert("coffee".to_owned())));
    let search = app.transition(Input::Intent(Intent::Action(Action::ChooseRichMedia)));
    assert_eq!(
        search.effect,
        Some(Effect::SearchPlaces {
            chat: ChatId(10),
            query: "coffee".to_owned(),
            near: None,
        })
    );
    assert!(
        search
            .view
            .rich_media
            .as_ref()
            .is_some_and(|view| view.pending)
    );

    let venue = PlaceView {
        point: GeoPointView {
            latitude_microdegrees: 31_230_400,
            longitude_microdegrees: 121_473_700,
        },
        title: "Coffee Lab".to_owned(),
        address: "1 Test Street".to_owned(),
        provider: "foursquare".to_owned(),
        venue_id: "venue-7".to_owned(),
        venue_type: "coffee".to_owned(),
    };
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::PlaceSearchReady {
            chat: ChatId(20),
            query: "coffee".to_owned(),
            near: None,
            places: vec![venue.clone()],
        }),
    );
    assert!(
        app.view()
            .rich_media
            .as_ref()
            .is_some_and(|view| view.pending)
    );
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::PlaceSearchReady {
            chat: ChatId(10),
            query: "coffee".to_owned(),
            near: None,
            places: vec![venue.clone()],
        }),
    );

    let send = app.transition(Input::Intent(Intent::Action(Action::ChooseRichMedia)));
    assert!(matches!(
        send.effect,
        Some(Effect::SendVenue {
            chat: ChatId(10),
            venue: observed,
            ..
        }) if observed == venue
    ));
}

fn opened_app() -> App {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
    );
    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
    app
}
