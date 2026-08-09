use test_harness::{Result, TelegramScenario, TestSystem, account, chat, key};

#[test]
fn mouse_positions_the_composer_cursor_and_invokes_visible_actions() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("pointer-composer-and-action")
        .terminal(100, 28)
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
    app.click_composer(6, 2)?;
    app.type_text("X")?;
    app.screen().composer().expect_text("first\nsecXond")?;

    app.click_action("Actions")?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Composer Actions"))
    );
    app.expect_no_unhandled_work()
}
