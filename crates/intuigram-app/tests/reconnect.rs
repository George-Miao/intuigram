use intuigram_lib::{Action, ConnectionState};
use test_harness::{Result, TelegramScenario, TestSystem, account, chat, incoming, key};

#[test]
fn reconnect_cooldown_does_not_block_composer_input() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("reconnect-responsive-composer")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [incoming(40, "Lin", "hello")])
                .expect_reconnect(),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.telegram().disconnect();
    app.screen()
        .expect_connection(ConnectionState::ReconnectCooldown)?;
    app.screen().action(Action::Reconnect).expect_available()?;

    app.type_text("still responsive")?;
    app.screen().composer().expect_text("still responsive")?;
    app.expect_saved_draft(10, "still responsive")?;

    app.press(key::ALT_RECONNECT)?;
    app.screen().expect_connection(ConnectionState::Connected)?;
    app.screen().composer().expect_text("still responsive")?;
    app.expect_no_unhandled_work()
}
