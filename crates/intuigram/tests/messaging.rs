use intuigram_lib::{AdapterEvent, ChatId, DeliveryState};
use intuigram_telegram::UpdateCursor;
use test_harness::{
    Result, TelegramScenario, TestSystem, account, chat, incoming, key, sent_message,
};

#[test]
fn pending_reply_is_acknowledged_after_telegram_completion() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("messaging-pending-reply")
        .terminal(100, 24)
        .time("2026-08-03T12:00:00Z")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [incoming(40, "Lin", "hello")])
                .hold_send_text("send", 10, "on it", Some(40)),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.screen().message(40).expect_active()?;
    app.choose_action("Reply")?;
    app.type_text("on it")?;
    app.press(key::ENTER)?;

    app.screen()
        .message_text("on it")
        .expect_delivery(DeliveryState::Pending)?;
    app.expect_saved_draft(10, "")?;

    app.telegram().complete("send", sent_message(41, "on it"))?;

    app.screen()
        .message(41)
        .expect_delivery(DeliveryState::Sent)?;
    app.expect_no_unhandled_work()
}

#[test]
fn rpc_confirmation_followed_by_its_live_update_keeps_one_message() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("messaging-confirmation-live-replay")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [])
                .hold_send_text("send", 10, "one copy", None),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.type_text("one copy")?;
    app.press(key::ENTER)?;
    let confirmed = sent_message(41, "one copy");
    app.telegram().complete("send", confirmed.clone())?;
    app.telegram().inject_update(
        UpdateCursor {
            pts: Some(1),
            pts_count: 1,
            ..UpdateCursor::default()
        },
        AdapterEvent::MessageAdded {
            chat: ChatId(10),
            message: Box::new(confirmed),
        },
    )?;

    app.screen()
        .message_text("one copy")
        .expect_delivery(DeliveryState::Sent)?;
    app.expect_no_unhandled_work()
}

#[test]
fn reconnect_does_not_restore_the_draft_consumed_by_a_pending_send() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("messaging-pending-send-suppresses-stale-draft")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [incoming(40, "Lin", "hello")])
                .hold_send_text("send", 10, "current message", None)
                .expect_load_history(10, [incoming(40, "Lin", "hello")]),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.type_text("current message")?;
    app.press(key::ENTER)?;
    app.telegram().restore(
        account("Ada")
            .with_chat(chat(10, "Rust"))
            .with_draft(10, "stale draft"),
    )?;

    app.screen().composer().expect_text("")?;
    app.screen()
        .message_text("current message")
        .expect_delivery(DeliveryState::Pending)?;

    app.telegram()
        .complete("send", sent_message(41, "current message"))?;
    app.expect_no_unhandled_work()
}
