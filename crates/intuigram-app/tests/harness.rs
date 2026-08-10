use intuigram_lib::{AdapterEvent, ChatId, DeliveryState};
use test_harness::{Error, Result, TelegramScenario, TestSystem, account, chat, incoming, key};

#[test]
fn locators_requery_the_latest_rendered_revision() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("harness-live-locator")
        .telegram(
            TelegramScenario::new()
                .bootstrap(
                    account("Ada")
                        .with_chat(chat(10, "First"))
                        .with_chat(chat(20, "Second")),
                )
                .expect_load_history(20, [incoming(50, "Lin", "second")]),
        )
        .start()?;
    let first = app.screen().chat("First");
    first.expect_active()?;

    app.press(key::DOWN)?;

    assert!(matches!(
        first.expect_active(),
        Err(Error::Expectation { .. })
    ));
    app.screen().chat("Second").expect_active()?;
    app.expect_no_unhandled_work()
}

#[test]
fn strict_telegram_mock_rejects_unexpected_work() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("harness-unexpected-command")
        .telegram(TelegramScenario::new().bootstrap(account("Ada").with_chat(chat(10, "Rust"))))
        .start()?;

    let error = app
        .press(key::ENTER)
        .expect_err("an unscripted history request must fail");
    assert!(matches!(error, Error::TelegramMismatch { .. }));
    Ok(())
}

#[test]
fn teardown_check_rejects_unused_expectations() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("harness-unused-command")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [incoming(40, "Lin", "hello")]),
        )
        .start()?;

    let error = app
        .expect_no_unhandled_work()
        .expect_err("unused strict expectations must fail teardown");
    assert!(matches!(error, Error::PendingWork { .. }));
    Ok(())
}

#[test]
fn paste_focus_and_resize_are_delivered_once_through_production_input() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("harness-terminal-events")
        .seed(7)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [incoming(40, "Lin", "hello")]),
        )
        .start()?;

    app.press(key::ENTER)?;
    let revision = app.screen().revision();
    app.paste("once")?;
    app.focus(false)?;
    app.focus(true)?;
    app.resize(120, 30)?;
    app.telegram().inject(AdapterEvent::MessageUpdated {
        chat: ChatId(10),
        message: Box::new(incoming(40, "Lin", "updated")),
    })?;

    app.screen().composer().expect_text("once")?;
    app.screen()
        .message(40)
        .expect_delivery(DeliveryState::Read)?;
    assert_eq!(app.screen().rows().len(), 30);
    assert!(app.screen().revision() > revision);
    app.expect_no_unhandled_work()
}
