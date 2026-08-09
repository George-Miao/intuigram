use super::*;

#[test]
fn unsupported_and_live_location_media_keep_informative_cards() {
    let unsupported = normalize_serialized_media(&tl::enums::MessageMedia::Unsupported.to_bytes())
        .expect("unsupported constructor should remain representable");
    assert_eq!(unsupported.kind, MediaKind::Unsupported);
    assert_eq!(unsupported.title, "Unsupported Content");
    assert!(!unsupported.description.is_empty());

    let live_location = tl::enums::MessageMedia::GeoLive(tl::types::MessageMediaGeoLive {
        geo: tl::enums::GeoPoint::Point(tl::types::GeoPoint {
            long: 139.6917,
            lat: 35.6895,
            access_hash: 1,
            accuracy_radius: Some(10),
        }),
        heading: None,
        period: 900,
        proximity_notification_radius: None,
    });
    let card = normalize_serialized_media(&live_location.to_bytes())
        .expect("live-location constructor should remain representable");
    assert_eq!(card.kind, MediaKind::LiveLocation);
    assert_eq!(card.display_description(), "35.689500, 139.691700");
    assert_eq!(
        card.display_details(),
        ["sharing for 15 min", "accuracy ±10 m"]
    );
    assert!(matches!(
        card.specialized,
        Some(SpecializedMediaView::LiveLocation(_))
    ));
}

#[test]
fn game_metadata_is_normalized_without_inventing_a_launch_target() {
    let media = tl::enums::MessageMedia::Game(tl::types::MessageMediaGame {
        game: tl::types::Game {
            id: 700,
            access_hash: 701,
            short_name: "orbit_runner".to_owned(),
            title: "Orbit Runner".to_owned(),
            description: "Pilot a completion-driven spacecraft".to_owned(),
            photo: tl::enums::Photo::Empty(tl::types::PhotoEmpty { id: 702 }),
            document: None,
        }
        .into(),
    });

    let card = normalize_serialized_media(&media.to_bytes())
        .expect("game constructor should remain representable");

    assert_eq!(card.kind, MediaKind::Game);
    assert_eq!(card.title, "Orbit Runner");
    assert_eq!(
        card.display_description(),
        "Pilot a completion-driven spacecraft"
    );
    assert_eq!(card.display_details(), ["game · orbit_runner"]);
    assert!(card.remote_id.is_none());
    assert!(matches!(
        card.specialized,
        Some(SpecializedMediaView::Game(_))
    ));
}

#[test]
fn invoice_metadata_is_normalized_without_a_purchase_action() {
    let media = tl::enums::MessageMedia::Invoice(Box::new(tl::types::MessageMediaInvoice {
        shipping_address_requested: true,
        test: false,
        title: "Conference ticket".to_owned(),
        description: "One-day admission".to_owned(),
        photo: None,
        receipt_msg_id: Some(70),
        currency: "USD".to_owned(),
        total_amount: 12_900,
        start_param: "conf-2026".to_owned(),
        extended_media: None,
    }));

    let card = normalize_serialized_media(&media.to_bytes())
        .expect("invoice constructor should remain representable");

    assert_eq!(card.kind, MediaKind::Invoice);
    assert_eq!(card.display_description(), "One-day admission");
    assert_eq!(
        card.display_details(),
        [
            "USD 12900 minor units".to_owned(),
            "receipt · message 70".to_owned(),
            "shipping address requested".to_owned(),
        ]
    );
    assert!(card.remote_id.is_none());
    assert!(matches!(
        card.specialized,
        Some(SpecializedMediaView::Invoice(_))
    ));
}

#[test]
fn paid_media_discloses_price_and_availability_without_a_purchase_action() {
    let preview = || {
        tl::enums::MessageExtendedMedia::Preview(tl::types::MessageExtendedMediaPreview {
            w: Some(640),
            h: Some(480),
            thumb: None,
            video_duration: None,
        })
    };
    let available =
        tl::enums::MessageExtendedMedia::Media(Box::new(tl::types::MessageExtendedMedia {
            media: tl::enums::MessageMedia::Photo(tl::types::MessageMediaPhoto {
                spoiler: false,
                live_photo: false,
                photo: Some(tl::enums::Photo::Empty(tl::types::PhotoEmpty { id: 71 })),
                ttl_seconds: None,
                video: None,
            }),
        }));
    let media = tl::enums::MessageMedia::PaidMedia(tl::types::MessageMediaPaidMedia {
        stars_amount: 50,
        extended_media: vec![preview(), preview(), available],
    });

    let card = normalize_serialized_media(&media.to_bytes())
        .expect("paid-media constructor should remain representable");

    assert_eq!(card.kind, MediaKind::PaidMedia);
    assert_eq!(card.display_description(), "50 Stars · 3 items");
    assert_eq!(
        card.display_details(),
        [
            "1. preview · 640×480",
            "2. preview · 640×480",
            "3. Photo · Photo",
        ]
    );
    assert!(card.remote_id.is_none());
    assert!(matches!(
        card.specialized,
        Some(SpecializedMediaView::PaidMedia(_))
    ));
}

#[test]
fn giveaway_launch_and_results_keep_distinct_state() {
    let launch = tl::enums::MessageMedia::Giveaway(tl::types::MessageMediaGiveaway {
        only_new_subscribers: true,
        winners_are_visible: true,
        channels: vec![10, 20],
        countries_iso2: Some(vec!["JP".to_owned(), "US".to_owned()]),
        prize_description: None,
        quantity: 3,
        months: Some(6),
        stars: None,
        until_date: 1_788_134_400,
    });
    let results =
        tl::enums::MessageMedia::GiveawayResults(tl::types::MessageMediaGiveawayResults {
            only_new_subscribers: true,
            refunded: true,
            channel_id: 10,
            additional_peers_count: Some(1),
            launch_msg_id: 50,
            winners_count: 2,
            unclaimed_count: 1,
            winners: vec![100, 200],
            months: Some(6),
            stars: None,
            prize_description: None,
            until_date: 1_788_134_400,
        });

    let launch = normalize_serialized_media(&launch.to_bytes())
        .expect("giveaway constructor should remain representable");
    let results = normalize_serialized_media(&results.to_bytes())
        .expect("giveaway-results constructor should remain representable");

    assert_eq!(launch.kind, MediaKind::Giveaway);
    assert_eq!(launch.display_description(), "3 winners · 6 months Premium");
    assert!(matches!(
        launch.specialized,
        Some(SpecializedMediaView::Giveaway(GiveawayView {
            state: GiveawayStateView::Active,
            ..
        }))
    ));
    assert_eq!(results.kind, MediaKind::Giveaway);
    assert_eq!(results.display_description(), "2 winners · 1 unclaimed");
    assert!(
        results
            .display_details()
            .iter()
            .any(|line| line.contains("refunded"))
    );
    assert!(matches!(
        results.specialized,
        Some(SpecializedMediaView::Giveaway(GiveawayView {
            state: GiveawayStateView::Results,
            ..
        }))
    ));
}

#[test]
fn gift_service_action_keeps_structured_value_and_text_fallback() {
    let action = tl::enums::MessageAction::GiftStars(tl::types::MessageActionGiftStars {
        currency: "USD".to_owned(),
        amount: 999,
        stars: 500,
        crypto_currency: None,
        crypto_amount: None,
        transaction_id: Some("tx-85".to_owned()),
    });

    let card = service_event_media(&action).expect("gift action should expose a Media Card");

    assert_eq!(service_event_description(&action), "Gifted Telegram Stars");
    assert_eq!(card.kind, MediaKind::Gift);
    assert_eq!(card.display_description(), "500 Stars");
    assert_eq!(
        card.display_details(),
        [
            "USD 999 minor units".to_owned(),
            "reference · tx-85".to_owned(),
        ]
    );
    assert!(matches!(
        card.specialized,
        Some(SpecializedMediaView::Gift(GiftView {
            kind: GiftKindView::Stars,
            ..
        }))
    ));
}

#[test]
fn shared_story_keeps_available_and_reference_only_state() {
    let story = tl::enums::StoryItem::Item(Box::new(tl::types::StoryItem {
        pinned: false,
        public: true,
        close_friends: true,
        min: false,
        noforwards: false,
        edited: false,
        contacts: false,
        selected_contacts: false,
        out: false,
        id: 12,
        date: 1_754_764_800,
        from_id: None,
        fwd_from: None,
        expire_date: 1_754_851_200,
        caption: Some("Compio from the summit".to_owned()),
        entities: None,
        media: tl::enums::MessageMedia::Empty,
        media_areas: None,
        privacy: None,
        views: None,
        sent_reaction: None,
        albums: None,
        music: None,
    }));
    let available = tl::enums::MessageMedia::Story(Box::new(tl::types::MessageMediaStory {
        via_mention: true,
        peer: tl::enums::Peer::User(tl::types::PeerUser { user_id: 77 }),
        id: 12,
        story: Some(story),
    }));
    let reference = tl::enums::MessageMedia::Story(Box::new(tl::types::MessageMediaStory {
        via_mention: false,
        peer: tl::enums::Peer::User(tl::types::PeerUser { user_id: 77 }),
        id: 13,
        story: None,
    }));

    let available = normalize_serialized_media(&available.to_bytes())
        .expect("shared Story should remain representable");
    let reference = normalize_serialized_media(&reference.to_bytes())
        .expect("Story reference should remain representable");

    assert_eq!(available.kind, MediaKind::Story);
    assert_eq!(available.display_description(), "Compio from the summit");
    assert!(matches!(
        available.specialized,
        Some(SpecializedMediaView::Story(SharedStoryView {
            state: StoryStateView::Available,
            close_friends: true,
            ..
        }))
    ));
    assert_eq!(reference.display_description(), "Story 13 not loaded");
    assert!(matches!(
        reference.specialized,
        Some(SpecializedMediaView::Story(SharedStoryView {
            state: StoryStateView::Reference,
            ..
        }))
    ));
}

#[test]
fn todo_list_keeps_ordered_items_completions_and_permissions() {
    let todo = tl::types::TodoList {
        others_can_append: true,
        others_can_complete: true,
        title: text_entities("Release checklist"),
        list: vec![
            tl::types::TodoItem {
                id: 1,
                title: text_entities("Run nextest"),
            }
            .into(),
            tl::types::TodoItem {
                id: 2,
                title: text_entities("Write release notes"),
            }
            .into(),
        ],
    }
    .into();
    let media = tl::enums::MessageMedia::ToDo(tl::types::MessageMediaToDo {
        todo,
        completions: Some(vec![
            tl::types::TodoCompletion {
                id: 1,
                completed_by: tl::enums::Peer::User(tl::types::PeerUser { user_id: 77 }),
                date: 1_754_764_800,
            }
            .into(),
        ]),
    });

    let card = normalize_serialized_media(&media.to_bytes())
        .expect("TODO-list constructor should remain representable");

    assert_eq!(card.kind, MediaKind::TodoList);
    assert_eq!(card.display_description(), "Release checklist");
    assert!(matches!(
        card.specialized,
        Some(SpecializedMediaView::TodoList(TodoListView {
            others_can_append: true,
            others_can_complete: true,
            ref items,
            ..
        })) if items.len() == 2 && items[0].completed && !items[1].completed
    ));
}

fn text_entities(text: &str) -> tl::enums::TextWithEntities {
    tl::types::TextWithEntities {
        text: text.to_owned(),
        entities: Vec::new(),
    }
    .into()
}
