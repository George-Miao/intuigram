use intuigram_app::ReactionView;
use test_harness::{Result, TelegramScenario, TestSystem, account, chat, key, sent_message};

#[test]
fn editing_an_outgoing_message_restores_the_existing_draft() -> Result<()> {
    let mut edited = sent_message(41, "new text");
    edited.details.edited = true;
    let mut app = TestSystem::builder()
        .name("message-actions-edit")
        .telegram(
            TelegramScenario::new()
                .bootstrap(
                    account("Ada")
                        .with_chat(chat(10, "Rust"))
                        .with_draft(10, "unfinished draft"),
                )
                .expect_load_history(10, [sent_message(41, "old text")])
                .expect_edit_message(10, 41, "new text", edited),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.press(key::ALT_EDIT)?;
    app.screen().composer().expect_focused()?;
    app.screen().composer().expect_text("old text")?;
    for _ in 0.."old text".chars().count() {
        app.press(key::BACKSPACE)?;
    }
    app.type_text("new text")?;
    app.press(key::ENTER)?;

    app.screen().message_text("new text").expect_active()?;
    app.screen().composer().expect_text("unfinished draft")?;
    app.expect_saved_draft(10, "unfinished draft")?;
    app.expect_no_unhandled_work()
}

#[test]
fn deleting_a_message_requires_confirmation_and_removes_it_durably() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("message-actions-delete")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [sent_message(41, "remove me")])
                .expect_delete_messages(10, [41]),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.press(key::ALT_DELETE)?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Delete Message 41?"))
    );
    app.screen().message(41).expect_active()?;
    app.press(key::ENTER)?;

    app.screen().message(41).expect_absent()?;
    app.expect_no_durable_message(10, 41)?;
    app.expect_no_unhandled_work()
}

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
    app.press(key::ALT_FORWARD)?;
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
    app.press(key::ALT_REACT)?;
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
