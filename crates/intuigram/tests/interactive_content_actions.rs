use intuigram_app::{
    GiveawayInfoView, GiveawayStateView, GiveawayView, LiveLocationView, MediaCard, MediaKind,
    PaidMediaItemView, PaidMediaView, SharedStoryView, SpecializedMediaView,
    SpecializedRefreshTarget, StoryStateView, TodoItemView, TodoListView,
};
use test_harness::{Result, TelegramScenario, TestSystem, account, chat, incoming, key};

#[test]
fn live_location_open_map_uses_the_confirmed_platform_link_pipeline() -> Result<()> {
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
            heading_degrees: None,
            period_seconds: 900,
            proximity_radius_metres: None,
            accuracy_radius_metres: None,
        })),
        remote_id: None,
    });
    let mut app = TestSystem::builder()
        .name("interactive-live-location-open-map")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [location]),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.choose_action("Open Map")?;

    assert_eq!(
        app.opened_links(),
        &["https://www.openstreetmap.org/?mlat=37.774900&mlon=-122.419400#map=16/37.774900/-122.419400"]
    );
    app.expect_no_unhandled_work()
}

#[test]
fn locked_paid_media_refreshes_without_exposing_a_purchase_action() -> Result<()> {
    let locked = paid_message(false);
    let available = paid_message(true);
    let mut app = TestSystem::builder()
        .name("interactive-paid-refresh")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [locked])
                .expect_refresh_specialized(10, 81, SpecializedRefreshTarget::PaidMedia, available),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.choose_action("Refresh")?;

    let rows = app.screen().rows();
    assert!(rows.iter().any(|row| row.contains("1. Photo · Photo")));
    assert!(rows.iter().all(|row| !row.contains("Buy")));
    app.expect_no_unhandled_work()
}

#[test]
fn todo_editor_toggles_a_selected_item_and_appends_plain_text() -> Result<()> {
    let initial = todo_message(false, false);
    let toggled = todo_message(true, false);
    let appended = todo_message(true, true);
    let mut app = TestSystem::builder()
        .name("interactive-todo-edit")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [initial])
                .expect_toggle_todo(10, 82, 1, true, toggled)
                .expect_append_todo(10, 82, "Ship it", appended),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.choose_action("Update TODO")?;
    app.press(key::SPACE)?;
    app.choose_action("Update TODO")?;
    app.type_text("a")?;
    app.type_text("Ship it")?;
    app.press(key::ENTER)?;

    let rows = app.screen().rows();
    assert!(rows.iter().any(|row| row.contains("☒ Verify release")));
    assert!(rows.iter().any(|row| row.contains("☐ Ship it")));
    app.expect_no_unhandled_work()
}

#[test]
fn story_refresh_replaces_a_reference_with_the_requested_payload() -> Result<()> {
    let reference = story_message(false);
    let available = story_message(true);
    let mut app = TestSystem::builder()
        .name("interactive-story-refresh")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [reference])
                .expect_refresh_specialized(
                    10,
                    84,
                    SpecializedRefreshTarget::Story {
                        peer: intuigram_app::ChatId(77),
                        id: 12,
                    },
                    available,
                ),
        )
        .start()?;
    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.choose_action("Refresh")?;

    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Release shipped"))
    );
    app.expect_no_unhandled_work()
}

#[test]
fn giveaway_refresh_discloses_participation_without_an_entry_purchase() -> Result<()> {
    let initial = giveaway_message(false);
    let refreshed = giveaway_message(true);
    let mut app = TestSystem::builder()
        .name("interactive-giveaway-refresh")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [initial])
                .expect_refresh_specialized(10, 85, SpecializedRefreshTarget::Giveaway, refreshed),
        )
        .start()?;
    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.choose_action("Refresh")?;

    let rows = app.screen().rows();
    assert!(rows.iter().any(|row| row.contains("participating")));
    assert!(rows.iter().all(|row| !row.contains("Buy")));
    app.expect_no_unhandled_work()
}

fn paid_message(available: bool) -> intuigram_app::MessageView {
    let mut message = incoming(81, "Lin", "[Paid media]");
    let item = if available {
        PaidMediaItemView::Available {
            kind: MediaKind::Photo,
            title: "Photo".to_owned(),
            remote_id: Some("photo-81".to_owned()),
        }
    } else {
        PaidMediaItemView::Preview {
            width: Some(640),
            height: Some(480),
            duration_seconds: None,
        }
    };
    message.details.media = Some(MediaCard {
        kind: MediaKind::PaidMedia,
        title: "Paid media".to_owned(),
        description: String::new(),
        details: Vec::new(),
        poll: None,
        specialized: Some(SpecializedMediaView::PaidMedia(PaidMediaView {
            stars_amount: 50,
            items: vec![item],
        })),
        remote_id: None,
    });
    message
}

fn todo_message(completed: bool, appended: bool) -> intuigram_app::MessageView {
    let mut message = incoming(82, "Lin", "[TODO list]");
    let mut items = vec![TodoItemView {
        id: 1,
        title: "Verify release".to_owned(),
        completed,
        completed_by: completed.then_some(intuigram_app::ChatId(77)),
        completed_date: completed.then_some("2026-08-10".to_owned()),
    }];
    if appended {
        items.push(TodoItemView {
            id: 2,
            title: "Ship it".to_owned(),
            completed: false,
            completed_by: None,
            completed_date: None,
        });
    }
    message.details.media = Some(MediaCard {
        kind: MediaKind::TodoList,
        title: "TODO list".to_owned(),
        description: String::new(),
        details: Vec::new(),
        poll: None,
        specialized: Some(SpecializedMediaView::TodoList(TodoListView {
            title: "Release".to_owned(),
            items,
            others_can_append: true,
            others_can_complete: true,
        })),
        remote_id: None,
    });
    message
}

fn story_message(available: bool) -> intuigram_app::MessageView {
    let mut message = incoming(84, "Lin", "[Shared Story]");
    message.details.media = Some(MediaCard {
        kind: MediaKind::Story,
        title: "Shared Story".to_owned(),
        description: String::new(),
        details: Vec::new(),
        poll: None,
        specialized: Some(SpecializedMediaView::Story(SharedStoryView {
            peer: intuigram_app::ChatId(77),
            id: 12,
            state: if available {
                StoryStateView::Available
            } else {
                StoryStateView::Reference
            },
            caption: available.then_some("Release shipped".to_owned()),
            date: String::new(),
            expires: String::new(),
            via_mention: true,
            close_friends: false,
            live: false,
        })),
        remote_id: None,
    });
    message
}

fn giveaway_message(refreshed: bool) -> intuigram_app::MessageView {
    let mut message = incoming(85, "Lin", "[Giveaway]");
    message.details.media = Some(MediaCard {
        kind: MediaKind::Giveaway,
        title: "Giveaway".to_owned(),
        description: String::new(),
        details: Vec::new(),
        poll: None,
        specialized: Some(SpecializedMediaView::Giveaway(GiveawayView {
            state: GiveawayStateView::Active,
            quantity: 3,
            premium_months: Some(6),
            stars: None,
            prize_description: None,
            until_date: "2026-08-31".to_owned(),
            only_new_subscribers: false,
            winners_visible: true,
            country_codes: Vec::new(),
            channel_count: 1,
            winners_count: None,
            unclaimed_count: None,
            refunded: false,
            info: refreshed.then_some(GiveawayInfoView::Active {
                participating: true,
                preparing_results: false,
                start_date: "2026-08-10".to_owned(),
                eligibility_issue: None,
            }),
        })),
        remote_id: None,
    });
    message
}
