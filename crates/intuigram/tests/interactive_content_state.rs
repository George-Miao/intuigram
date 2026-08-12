use intuigram_lib::{
    GiftKindView, GiftView, GiveawayStateView, GiveawayView, MediaCard, MediaKind,
    PaidMediaItemView, PaidMediaView, SharedStoryView, SpecializedMediaView, StoryStateView,
    TodoItemView, TodoListView,
};
use test_harness::{Result, TelegramScenario, TestSystem, account, chat, incoming, key};

#[test]
fn paid_media_renders_price_and_availability_without_a_purchase_action() -> Result<()> {
    let mut paid = incoming(83, "Lin", "[Paid media]");
    paid.details.media = Some(MediaCard {
        kind: MediaKind::PaidMedia,
        title: "Paid media".to_owned(),
        description: String::new(),
        details: Vec::new(),
        poll: None,
        specialized: Some(SpecializedMediaView::PaidMedia(PaidMediaView {
            stars_amount: 50,
            items: vec![
                PaidMediaItemView::Preview {
                    width: Some(640),
                    height: Some(480),
                    duration_seconds: None,
                },
                PaidMediaItemView::Preview {
                    width: Some(320),
                    height: Some(240),
                    duration_seconds: Some(12),
                },
                PaidMediaItemView::Available {
                    kind: MediaKind::Photo,
                    title: "Photo".to_owned(),
                    remote_id: None,
                },
            ],
        })),
        remote_id: None,
    });
    let mut app = TestSystem::builder()
        .name("interactive-paid-media")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [paid]),
        )
        .start()?;

    app.press(key::ENTER)?;

    app.screen()
        .media_card("Paid media")
        .expect_description("50 Stars · 3 items")?;
    let rows = app.screen().rows();
    assert!(rows.iter().any(|row| row.contains("1. preview · 640×480")));
    assert!(
        rows.iter()
            .any(|row| row.contains("2. preview · 320×240 · 12s"))
    );
    assert!(rows.iter().any(|row| row.contains("3. Photo · Photo")));
    assert!(rows.iter().all(|row| !row.contains("Buy")));
    app.expect_no_unhandled_work()
}

#[test]
fn giveaway_renders_prize_deadline_and_eligibility() -> Result<()> {
    let mut giveaway = incoming(84, "Lin", "[Giveaway]");
    giveaway.details.media = Some(MediaCard {
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
            only_new_subscribers: true,
            winners_visible: true,
            country_codes: vec!["JP".to_owned(), "US".to_owned()],
            channel_count: 2,
            winners_count: None,
            unclaimed_count: None,
            refunded: false,
            info: None,
        })),
        remote_id: None,
    });
    let mut app = TestSystem::builder()
        .name("interactive-giveaway")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [giveaway]),
        )
        .start()?;

    app.press(key::ENTER)?;

    app.screen()
        .media_card("Giveaway")
        .expect_description("3 winners · 6 months Premium")?;
    let rows = app.screen().rows();
    assert!(rows.iter().any(|row| row.contains("ends 2026-08-31")));
    assert!(
        rows.iter()
            .any(|row| row.contains("new subscribers · winners visible"))
    );
    assert!(rows.iter().any(|row| row.contains("JP, US · 2 channels")));
    app.expect_no_unhandled_work()
}

#[test]
fn gift_service_message_keeps_value_and_delivery_state() -> Result<()> {
    let mut gift = incoming(85, "Telegram", "Gifted Telegram Stars");
    gift.details.service = Some("Gifted Telegram Stars".to_owned());
    gift.details.media = Some(MediaCard {
        kind: MediaKind::Gift,
        title: "Stars gift".to_owned(),
        description: String::new(),
        details: Vec::new(),
        poll: None,
        specialized: Some(SpecializedMediaView::Gift(GiftView {
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
        })),
        remote_id: None,
    });
    let mut app = TestSystem::builder()
        .name("interactive-gift")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [gift]),
        )
        .start()?;

    app.press(key::ENTER)?;

    app.screen()
        .media_card("Stars gift")
        .expect_description("500 Stars")?;
    let rows = app.screen().rows();
    assert!(rows.iter().any(|row| row.contains("USD 999 minor units")));
    assert!(rows.iter().any(|row| row.contains("reference · tx-85")));
    app.expect_no_unhandled_work()
}

#[test]
fn shared_story_renders_source_caption_and_expiry_state() -> Result<()> {
    let mut story = incoming(86, "Lin", "[Shared Story]");
    story.details.media = Some(MediaCard {
        kind: MediaKind::Story,
        title: "Shared Story".to_owned(),
        description: String::new(),
        details: Vec::new(),
        poll: None,
        specialized: Some(SpecializedMediaView::Story(SharedStoryView {
            peer: intuigram_lib::ChatId(77),
            id: 12,
            state: StoryStateView::Available,
            caption: Some("Compio from the summit".to_owned()),
            date: "2026-08-10".to_owned(),
            expires: "2026-08-11".to_owned(),
            via_mention: true,
            close_friends: true,
            live: false,
        })),
        remote_id: None,
    });
    let mut app = TestSystem::builder()
        .name("interactive-shared-story")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [story]),
        )
        .start()?;

    app.press(key::ENTER)?;

    app.screen()
        .media_card("Shared Story")
        .expect_description("Compio from the summit")?;
    let rows = app.screen().rows();
    assert!(rows.iter().any(|row| row.contains("peer 77 · story 12")));
    assert!(
        rows.iter()
            .any(|row| row.contains("2026-08-10 · expires 2026-08-11"))
    );
    assert!(
        rows.iter()
            .any(|row| row.contains("mention · close friends"))
    );
    app.expect_no_unhandled_work()
}

#[test]
fn todo_list_renders_ordered_completion_and_permissions() -> Result<()> {
    let mut todo = incoming(87, "Lin", "[TODO] Release checklist");
    todo.details.media = Some(MediaCard {
        kind: MediaKind::TodoList,
        title: "TODO list".to_owned(),
        description: String::new(),
        details: Vec::new(),
        poll: None,
        specialized: Some(SpecializedMediaView::TodoList(TodoListView {
            title: "Release checklist".to_owned(),
            items: vec![
                TodoItemView {
                    id: 1,
                    title: "Run nextest".to_owned(),
                    completed: true,
                    completed_by: Some(intuigram_lib::ChatId(77)),
                    completed_date: Some("2026-08-10".to_owned()),
                },
                TodoItemView {
                    id: 2,
                    title: "Write release notes".to_owned(),
                    completed: false,
                    completed_by: None,
                    completed_date: None,
                },
            ],
            others_can_append: true,
            others_can_complete: true,
        })),
        remote_id: None,
    });
    let mut app = TestSystem::builder()
        .name("interactive-todo-list")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [todo]),
        )
        .start()?;

    app.press(key::ENTER)?;

    app.screen()
        .media_card("TODO list")
        .expect_description("Release checklist")?;
    let rows = app.screen().rows();
    assert!(
        rows.iter()
            .any(|row| row.contains("☒ Run nextest · peer 77 · 2026-08-10"))
    );
    assert!(rows.iter().any(|row| row.contains("☐ Write release notes")));
    assert!(
        rows.iter()
            .any(|row| row.contains("members may add · members may complete"))
    );
    app.expect_no_unhandled_work()
}
