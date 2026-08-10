use intuigram_lib::{AdapterEvent, ChatId};
use intuigram_telegram::UpdateCursor;
use test_harness::{Result, TelegramScenario, TestSystem, account, chat, incoming, key};

#[test]
fn live_message_is_committed_with_its_cursor_before_it_is_rendered() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("synchronization-durable-before-render")
        .terminal(100, 24)
        .telegram(TelegramScenario::new().bootstrap(account("Ada").with_chat(chat(10, "Rust"))))
        .start()?;

    let message = incoming(52, "Lin", "durable before visible");
    app.telegram().inject_update(
        UpdateCursor {
            pts: Some(19),
            date: Some(1_786_000_000),
            seq: Some(7),
            seq_start: Some(7),
            ..UpdateCursor::default()
        },
        AdapterEvent::MessageAdded {
            chat: ChatId(10),
            message: Box::new(message),
        },
    )?;

    app.screen()
        .message_text("durable before visible")
        .expect_sender("Lin")?;
    app.expect_durable_message(10, 52, "durable before visible")?;
    app.expect_sync_cursor("account", 19, 0, 1_786_000_000, 7)?;
    app.expect_no_unhandled_work()
}

#[test]
fn a_replayed_live_update_does_not_duplicate_loaded_history() -> Result<()> {
    let loaded = incoming(52, "Lin", "loaded once");
    let mut app = TestSystem::builder()
        .name("synchronization-replayed-message")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [loaded.clone()]),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.telegram().inject_update(
        UpdateCursor {
            pts: Some(19),
            pts_count: 1,
            ..UpdateCursor::default()
        },
        AdapterEvent::MessageAdded {
            chat: ChatId(10),
            message: Box::new(loaded),
        },
    )?;

    app.screen().message(52).expect_sender("Lin")?;
    app.expect_no_unhandled_work()
}

#[test]
fn a_refresh_removes_stale_acknowledged_messages_from_screen_and_storage() -> Result<()> {
    let older = incoming(1, "Lin", "older retained history");
    let stale = incoming(8, "You", "deleted elsewhere");
    let mut app = TestSystem::builder()
        .name("synchronization-prunes-stale-history")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(
                    account("Ada")
                        .with_chat(chat(10, "Rust"))
                        .with_history([older.clone(), stale]),
                )
                .expect_load_history(
                    10,
                    [
                        older,
                        incoming(7, "Lin", "fresh history"),
                        incoming(10, "Lin", "latest"),
                    ],
                ),
        )
        .start()?;

    app.screen()
        .message_text("deleted elsewhere")
        .expect_sender("You")?;
    app.press(key::ENTER)?;

    app.screen().message(8).expect_absent()?;
    app.expect_no_durable_message(10, 8)?;
    app.expect_no_unhandled_work()
}

#[test]
fn a_chat_discovered_live_can_immediately_load_history() -> Result<()> {
    let discovered = 206_899_663;
    let mut app = TestSystem::builder()
        .name("synchronization-live-chat-remains-operable")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(discovered, [incoming(53, "Peer", "complete history")]),
        )
        .start()?;

    app.telegram().inject_update(
        UpdateCursor {
            pts: Some(20),
            pts_count: 1,
            ..UpdateCursor::default()
        },
        AdapterEvent::MessageAdded {
            chat: ChatId(discovered),
            message: Box::new(incoming(52, "Peer", "new Chat")),
        },
    )?;
    app.press(key::DOWN)?;

    app.screen()
        .chat(format!("Chat {discovered}"))
        .expect_active()?;
    app.screen()
        .message_text("complete history")
        .expect_sender("Peer")?;
    app.expect_no_unhandled_work()
}
