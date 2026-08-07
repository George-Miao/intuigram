use intuigram_app::{AdapterEvent, ChatId};
use test_harness::{
    Result, TelegramScenario, TestSystem, account, chat, incoming, key, sent_message,
};

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

#[test]
fn default_keys_follow_the_chat_composer_message_hierarchy() -> Result<()> {
    let mut archived = chat(20, "Archived");
    archived.folders = vec![-1];
    let mut app = TestSystem::builder()
        .name("navigation-default-hierarchy-keys")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(
                    account("Ada")
                        .with_chat(chat(10, "Rust"))
                        .with_chat(archived),
                )
                .expect_load_history(20, [incoming(50, "Mira", "archived message")])
                .expect_load_history(
                    10,
                    [
                        incoming(40, "Lin", "older message"),
                        incoming(41, "Lin", "newer message"),
                    ],
                ),
        )
        .start()?;

    app.screen().folder("All").expect_active()?;
    app.press(key::RIGHT)?;
    app.screen().folder("Archive").expect_active()?;
    app.screen().chat("Archived").expect_active()?;
    app.press(key::LEFT)?;
    app.screen().folder("All").expect_active()?;
    app.screen().chat("Rust").expect_active()?;

    app.press(key::ENTER)?;
    app.type_text("draft")?;
    app.screen().composer().expect_focused()?;
    app.screen().composer().expect_text("draft")?;
    app.press(key::ALT_UP)?;
    app.screen().message(41).expect_active()?;
    app.press(key::UP)?;
    app.screen().message(40).expect_active()?;
    app.press(key::DOWN)?;
    app.screen().message(41).expect_active()?;
    app.press(key::ESCAPE)?;
    app.screen().composer().expect_focused()?;
    app.screen().composer().expect_text("draft")?;
    app.expect_no_unhandled_work()
}

#[test]
fn empty_composer_up_edits_the_newest_eligible_outgoing_message() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("navigation-edit-previous-message")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(
                    10,
                    [
                        sent_message(40, "editable message"),
                        incoming(41, "Lin", "newer incoming message"),
                    ],
                ),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::UP)?;

    app.screen().composer().expect_focused()?;
    app.screen().composer().expect_text("editable message")?;
    app.screen()
        .action(intuigram_app::Action::SaveEdit)
        .expect_available()?;
    app.expect_no_unhandled_work()
}

#[test]
fn empty_composer_up_does_nothing_without_an_editable_message() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("navigation-no-previous-message")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [incoming(41, "Lin", "incoming only")]),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::UP)?;

    app.screen().composer().expect_focused()?;
    app.screen().composer().expect_text("")?;
    app.expect_no_unhandled_work()
}

#[test]
fn nonempty_composer_up_moves_the_insertion_cursor() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("navigation-composer-cursor-up")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, []),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.type_text("first")?;
    app.press(key::SHIFT_ENTER)?;
    app.type_text("second")?;
    app.press(key::UP)?;
    app.type_text("X")?;

    app.screen().composer().expect_focused()?;
    app.screen().composer().expect_text("firstX\nsecond")?;
    app.expect_no_unhandled_work()
}
