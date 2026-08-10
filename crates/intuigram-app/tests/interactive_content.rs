use intuigram_lib::{
    GameView, InvoiceView, LiveLocationView, MediaCard, MediaKind, MessageId, SpecializedMediaView,
};
use test_harness::{Result, TelegramScenario, TestSystem, account, chat, incoming, key};

#[test]
fn live_location_renders_coordinates_and_sharing_state() -> Result<()> {
    let mut location = incoming(80, "Lin", "[Live location]");
    location.details.media = Some(MediaCard {
        kind: MediaKind::LiveLocation,
        title: "Live location".to_owned(),
        description: String::new(),
        details: Vec::new(),
        poll: None,
        specialized: Some(SpecializedMediaView::LiveLocation(LiveLocationView {
            latitude_microdegrees: 37_774_900,
            longitude_microdegrees: -122_419_400,
            heading_degrees: Some(90),
            period_seconds: 900,
            proximity_radius_metres: Some(25),
            accuracy_radius_metres: None,
        })),
        remote_id: None,
    });
    let mut app = TestSystem::builder()
        .name("interactive-live-location")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [location]),
        )
        .start()?;

    app.press(key::ENTER)?;

    app.screen()
        .media_card("Live location")
        .expect_description("37.774900, -122.419400")?;
    let rows = app.screen().rows();
    assert!(rows.iter().any(|row| row.contains("sharing for 15 min")));
    assert!(
        rows.iter()
            .any(|row| row.contains("heading 90° · within 25 m"))
    );
    app.expect_no_unhandled_work()
}

#[test]
fn game_renders_its_title_description_and_short_name() -> Result<()> {
    let mut game = incoming(81, "Lin", "[Game] Orbit Runner");
    game.details.media = Some(MediaCard {
        kind: MediaKind::Game,
        title: "Orbit Runner".to_owned(),
        description: String::new(),
        details: Vec::new(),
        poll: None,
        specialized: Some(SpecializedMediaView::Game(GameView {
            id: 700,
            short_name: "orbit_runner".to_owned(),
            title: "Orbit Runner".to_owned(),
            description: "Pilot a completion-driven spacecraft".to_owned(),
        })),
        remote_id: None,
    });
    let mut app = TestSystem::builder()
        .name("interactive-game")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [game]),
        )
        .start()?;

    app.press(key::ENTER)?;

    app.screen()
        .media_card("Orbit Runner")
        .expect_description("Pilot a completion-driven spacecraft")?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("game · orbit_runner"))
    );
    app.expect_no_unhandled_work()
}

#[test]
fn invoice_renders_amount_and_receipt_state_without_a_purchase_action() -> Result<()> {
    let mut invoice = incoming(82, "Lin", "[Invoice] Conference ticket");
    invoice.details.media = Some(MediaCard {
        kind: MediaKind::Invoice,
        title: "Conference ticket".to_owned(),
        description: String::new(),
        details: Vec::new(),
        poll: None,
        specialized: Some(SpecializedMediaView::Invoice(InvoiceView {
            title: "Conference ticket".to_owned(),
            description: "One-day admission".to_owned(),
            currency: "USD".to_owned(),
            total_minor_units: 12_900,
            receipt_message: Some(MessageId(70)),
            shipping_address_requested: false,
            test: false,
            extended_media: false,
        })),
        remote_id: None,
    });
    let mut app = TestSystem::builder()
        .name("interactive-invoice")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [invoice]),
        )
        .start()?;

    app.press(key::ENTER)?;

    app.screen()
        .media_card("Conference ticket")
        .expect_description("One-day admission")?;
    let rows = app.screen().rows();
    assert!(rows.iter().any(|row| row.contains("USD 12900 minor units")));
    assert!(rows.iter().any(|row| row.contains("receipt · message 70")));
    assert!(rows.iter().all(|row| !row.contains("Pay")));
    app.expect_no_unhandled_work()
}
