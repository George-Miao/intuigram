use intuigram_app::{AdapterEvent, ChatId, MessageId};
use intuigram_telegram::{UpdateCursor, UpdateScope};
use test_harness::{Result, TelegramScenario, TestSystem, account, chat, incoming, key};

#[test]
fn pinned_messages_are_shown_and_navigated_without_losing_the_transcript_anchor() -> Result<()> {
    let mut rules = incoming(40, "Lin", "read the rules");
    rules.details.pinned = true;
    let middle = incoming(41, "Lin", "keep my place");
    let newest = incoming(42, "Lin", "latest");
    let mut app = TestSystem::builder()
        .name("pinned-messages-navigation")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [rules.clone(), middle.clone(), newest.clone()]),
        )
        .start()?;

    app.press(key::ENTER)?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Pinned · read the rules"))
    );
    app.press(key::PINNED)?;
    app.screen().message(40).expect_active()?;

    app.press(key::ALT_DOWN)?;
    app.screen().message(41).expect_active()?;
    app.telegram().inject_update(
        UpdateCursor {
            scope: UpdateScope::Account,
            pts: Some(1),
            pts_count: 1,
            ..UpdateCursor::default()
        },
        AdapterEvent::MessagesPinChanged {
            chat: ChatId(10),
            ids: vec![MessageId(newest.id.0)],
            pinned: true,
        },
    )?;

    app.screen().message(41).expect_active()?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Pinned · latest"))
    );
    app.expect_durable_message_pinned(10, 42, true)?;
    app.press(key::PINNED)?;
    app.screen().message(42).expect_active()?;
    app.expect_no_unhandled_work()
}

#[test]
fn active_cloud_messages_can_be_pinned_and_unpinned() -> Result<()> {
    let message = incoming(50, "Lin", "pin this");
    let mut pinned = message.clone();
    pinned.details.pinned = true;
    let mut unpinned = pinned.clone();
    unpinned.details.pinned = false;
    let mut app = TestSystem::builder()
        .name("pin-and-unpin-message")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [message])
                .expect_set_message_pinned(10, 50, true, pinned)
                .expect_set_message_pinned(10, 50, false, unpinned),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.press(key::ALT_PIN)?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Pinned · pin this"))
    );
    app.press(key::ALT_PIN)?;
    assert!(
        !app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Pinned · pin this"))
    );
    app.expect_no_unhandled_work()
}
