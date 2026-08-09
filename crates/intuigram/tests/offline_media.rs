use test_harness::{Result, TelegramScenario, TestSystem, account, chat, key};

#[test]
fn chat_actions_toggle_durable_offline_media_retention() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("offline-media-chat-policy")
        .terminal(100, 24)
        .telegram(TelegramScenario::new().bootstrap(account("Ada").with_chat(chat(10, "Rust"))))
        .start()?;

    app.press(key::ALT_ACTIONS)?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Keep Media Offline"))
    );
    app.press(key::ENTER)?;

    app.press(key::ALT_ACTIONS)?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Use Cache Eviction"))
    );
    app.press(key::ESCAPE)?;
    app.expect_no_unhandled_work()
}
