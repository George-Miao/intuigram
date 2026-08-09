use intuigram_app::{
    ChatId, DeliveryState, GameView, GiftKindView, GiftView, GiveawayStateView, GiveawayView,
    InvoiceView, LiveLocationView, MediaCard, MediaKind, MessageDetails, MessageDirection,
    MessageId, MessageView, PaidMediaItemView, PaidMediaView, SharedStoryView,
    SpecializedMediaView, StoryStateView, TodoItemView, TodoListView,
};

use super::super::{decode_stored_message, encode_stored_message};

#[test]
fn live_location_round_trips_through_the_cache() {
    assert_roundtrip(
        43,
        MediaKind::LiveLocation,
        "Live location",
        SpecializedMediaView::LiveLocation(LiveLocationView {
            latitude_microdegrees: 35_689_500,
            longitude_microdegrees: 139_691_700,
            heading_degrees: Some(180),
            period_seconds: 900,
            proximity_radius_metres: Some(50),
            accuracy_radius_metres: Some(10),
        }),
        "livelocation",
    );
}

#[test]
fn game_round_trips_through_the_cache() {
    assert_roundtrip(
        44,
        MediaKind::Game,
        "Orbit Runner",
        SpecializedMediaView::Game(GameView {
            id: 700,
            short_name: "orbit_runner".to_owned(),
            title: "Orbit Runner".to_owned(),
            description: "Pilot a completion-driven spacecraft".to_owned(),
        }),
        "game",
    );
}

#[test]
fn invoice_round_trips_through_the_cache() {
    assert_roundtrip(
        45,
        MediaKind::Invoice,
        "Conference ticket",
        SpecializedMediaView::Invoice(InvoiceView {
            title: "Conference ticket".to_owned(),
            description: "One-day admission".to_owned(),
            currency: "USD".to_owned(),
            total_minor_units: 12_900,
            receipt_message: Some(MessageId(70)),
            shipping_address_requested: true,
            test: false,
            extended_media: true,
        }),
        "invoice",
    );
}

#[test]
fn paid_media_round_trips_through_the_cache() {
    assert_roundtrip(
        46,
        MediaKind::PaidMedia,
        "Paid media",
        SpecializedMediaView::PaidMedia(PaidMediaView {
            stars_amount: 50,
            items: vec![
                PaidMediaItemView::Preview {
                    width: Some(640),
                    height: Some(480),
                    duration_seconds: None,
                },
                PaidMediaItemView::Available {
                    kind: MediaKind::Photo,
                    title: "Photo".to_owned(),
                    remote_id: Some("photo-71".to_owned()),
                },
            ],
        }),
        "paidmedia",
    );
}

#[test]
fn giveaway_round_trips_through_the_cache() {
    assert_roundtrip(
        47,
        MediaKind::Giveaway,
        "Giveaway",
        SpecializedMediaView::Giveaway(GiveawayView {
            state: GiveawayStateView::Active,
            quantity: 3,
            premium_months: Some(6),
            stars: None,
            prize_description: None,
            until_date: "2026-08-31".to_owned(),
            only_new_subscribers: true,
            winners_visible: true,
            country_codes: vec!["JP".to_owned(), "US".to_owned()],
            channel_count: 2,
            winners_count: None,
            unclaimed_count: None,
            refunded: false,
            info: None,
        }),
        "giveaway",
    );
}

#[test]
fn gift_round_trips_through_the_cache() {
    assert_roundtrip(
        48,
        MediaKind::Gift,
        "Stars gift",
        SpecializedMediaView::Gift(GiftView {
            kind: GiftKindView::Stars,
            title: "Stars gift".to_owned(),
            stars: Some(500),
            days: None,
            currency: Some("USD".to_owned()),
            amount_minor_units: Some(999),
            crypto_currency: None,
            crypto_amount_minor_units: None,
            identifier: Some("tx-85".to_owned()),
            saved: false,
            converted: false,
            upgraded: false,
            refunded: false,
            anonymous: false,
        }),
        "gift",
    );
}

#[test]
fn shared_story_round_trips_through_the_cache() {
    assert_roundtrip(
        49,
        MediaKind::Story,
        "Shared Story",
        SpecializedMediaView::Story(SharedStoryView {
            peer: ChatId(77),
            id: 12,
            state: StoryStateView::Available,
            caption: Some("Compio from the summit".to_owned()),
            date: "2026-08-10".to_owned(),
            expires: "2026-08-11".to_owned(),
            via_mention: true,
            close_friends: true,
            live: false,
        }),
        "story",
    );
}

#[test]
fn todo_list_round_trips_through_the_cache() {
    assert_roundtrip(
        50,
        MediaKind::TodoList,
        "TODO list",
        SpecializedMediaView::TodoList(TodoListView {
            title: "Release checklist".to_owned(),
            items: vec![TodoItemView {
                id: 1,
                title: "Run nextest".to_owned(),
                completed: true,
                completed_by: Some(ChatId(77)),
                completed_date: Some("2026-08-10".to_owned()),
            }],
            others_can_append: true,
            others_can_complete: true,
        }),
        "todolist",
    );
}

fn assert_roundtrip(
    id: i64,
    kind: MediaKind,
    title: &str,
    specialized: SpecializedMediaView,
    expected_content_kind: &str,
) {
    let message = MessageView {
        id: MessageId(id),
        sender: "Ada".to_owned(),
        body: format!("[{title}]"),
        timestamp: "12:00".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: None,
        details: MessageDetails {
            media: Some(MediaCard {
                kind,
                title: title.to_owned(),
                description: String::new(),
                details: Vec::new(),
                poll: None,
                specialized: Some(specialized),
                remote_id: None,
            }),
            ..MessageDetails::default()
        },
    };

    let stored = encode_stored_message(ChatId(7), &message);

    assert_eq!(stored.content_kind, expected_content_kind);
    assert_eq!(decode_stored_message(stored), message);
}
