use intuigram_app::{AdapterEvent, ChatId};
use test_harness::{Result, TelegramScenario, TestSystem, account, chat, incoming, key};

#[test]
fn opening_a_chat_loads_history_and_focuses_the_composer() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("navigation-open-chat")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [incoming(40, "Lin", "hello")]),
        )
        .start()?;

    app.screen().chat("Rust").expect_active()?;
    app.press(key::ENTER)?;

    app.screen().composer().expect_focused()?;
    app.screen()
        .message(40)
        .expect_delivery(intuigram_app::DeliveryState::Read)?;
    assert!(app.screen().rows().iter().any(|row| row.contains("hello")));
    app.expect_no_unhandled_work()
}

#[test]
fn archiving_the_active_chat_rebinds_the_transcript_to_the_replacement_chat() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("navigation-archive-rebinds-active-chat")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(
                    account("Ada")
                        .with_chat(chat(10, "Rust"))
                        .with_chat(chat(20, "Design")),
                )
                .expect_load_history(20, [incoming(50, "Mira", "replacement Chat history")])
                .expect_load_history(10, [incoming(40, "Lin", "old Chat history")]),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.screen()
        .message_text("old Chat history")
        .expect_sender("Lin")?;
    app.press(key::ESCAPE)?;

    app.telegram().inject(AdapterEvent::ChatArchiveChanged {
        chat: ChatId(10),
        archived: true,
    })?;

    app.screen().chat("Design").expect_active()?;
    app.screen()
        .message_text("replacement Chat history")
        .expect_sender("Mira")?;
    app.expect_no_unhandled_work()
}

#[test]
fn inactive_chat_history_is_loaded_before_the_chat_is_selected() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("navigation-background-chat-history")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(
                    account("Ada")
                        .with_chat(chat(10, "Rust"))
                        .with_chat(chat(20, "Design")),
                )
                .expect_load_history(20, [incoming(50, "Mira", "already loaded")]),
        )
        .start()?;

    app.expect_no_unhandled_work()?;
    app.press(key::DOWN)?;

    app.screen().chat("Design").expect_active()?;
    app.screen()
        .message_text("already loaded")
        .expect_sender("Mira")?;
    app.expect_no_unhandled_work()
}
