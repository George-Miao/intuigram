use intuigram_lib::ReactionView;
use test_harness::{Result, TelegramScenario, TestSystem, account, chat, key, sent_message};

#[test]
fn forwarding_uses_a_contextual_chat_picker() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("message-actions-forward")
        .telegram(
            TelegramScenario::new()
                .bootstrap(
                    account("Ada")
                        .with_chat(chat(10, "Rust"))
                        .with_chat(chat(20, "Lin")),
                )
                .expect_load_history(20, [])
                .expect_load_history(10, [sent_message(41, "share this")])
                .expect_forward_message(10, 20, 41),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.choose_action("Forward")?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Forward Message 41"))
    );
    app.press(key::ENTER)?;

    assert!(
        !app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Forward Message 41"))
    );
    app.expect_no_unhandled_work()
}

#[test]
fn forwarding_applies_to_the_message_selection_in_transcript_order() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("message-actions-batch-forward")
        .telegram(
            TelegramScenario::new()
                .bootstrap(
                    account("Ada")
                        .with_chat(chat(10, "Rust"))
                        .with_chat(chat(20, "Lin")),
                )
                .expect_load_history(20, [])
                .expect_load_history(10, [sent_message(41, "first"), sent_message(42, "second")])
                .expect_forward_messages(10, 20, [41, 42]),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.choose_action("Select Message")?;
    app.press(key::UP)?;
    app.choose_action("Select Message")?;
    app.choose_action("Forward")?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Forward 2 Messages"))
    );
    app.press(key::ENTER)?;

    assert!(!selected_message_is_visible(&app, "first"));
    assert!(!selected_message_is_visible(&app, "second"));
    app.expect_no_unhandled_work()
}

#[test]
fn reacting_uses_a_small_contextual_picker_and_persists_the_result() -> Result<()> {
    let mut reacted = sent_message(41, "nice");
    reacted.details.reactions.push(ReactionView {
        label: "👍".to_owned(),
        count: 1,
        chosen: true,
    });
    let mut app = TestSystem::builder()
        .name("message-actions-reaction")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [sent_message(41, "nice")])
                .expect_react_message(10, 41, "👍", reacted),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.choose_action("React")?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("React to Message 41"))
    );
    app.press(key::ENTER)?;

    app.screen().message(41).expect_reaction("👍", 1, true)?;
    app.expect_durable_message(10, 41, "nice")?;
    app.expect_no_unhandled_work()
}

fn selected_message_is_visible(app: &TestSystem, body: &str) -> bool {
    app.screen()
        .rows()
        .iter()
        .any(|row| row.contains("[✓]") && row.contains(body))
}
