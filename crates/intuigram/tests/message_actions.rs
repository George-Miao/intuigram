use intuigram_app::{Action, DeliveryState, ReactionView};
use test_harness::{Result, TelegramScenario, TestSystem, account, chat, key, sent_message};

#[test]
fn active_message_actions_are_grouped_in_one_selectable_popup() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("message-actions-popup")
        .terminal(100, 32)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [sent_message(41, "action target")]),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.screen()
        .action(Action::OpenActions)
        .expect_available()?;
    for action in [
        Action::Reply,
        Action::Edit,
        Action::Delete,
        Action::Forward,
        Action::React,
    ] {
        app.screen().action(action).expect_unavailable()?;
    }
    app.type_text("a")?;

    let popup = app.screen().rows().join("\n");
    for label in [
        "Message Actions",
        "Reply",
        "Edit",
        "Delete",
        "Forward",
        "React",
        "Open Thread",
        "Pin / Unpin",
        "Select Message",
    ] {
        assert!(popup.contains(label), "missing {label:?} in {popup:?}");
    }

    app.press(key::ENTER)?;
    app.screen().composer().expect_focused()?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Reply to 41"))
    );
    app.expect_no_unhandled_work()
}

#[test]
fn saving_an_edit_returns_to_an_empty_composer() -> Result<()> {
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
    choose_message_action(&mut app, "Edit")?;
    app.screen().composer().expect_focused()?;
    app.screen().composer().expect_text("old text")?;
    for _ in 0.."old text".chars().count() {
        app.press(key::BACKSPACE)?;
    }
    app.type_text("new text")?;
    app.press(key::ENTER)?;

    app.screen()
        .message_text("new text")
        .expect_delivery(DeliveryState::Sent)?;
    app.screen().composer().expect_focused()?;
    app.screen().composer().expect_text("")?;
    app.expect_saved_draft(10, "")?;
    app.expect_no_unhandled_work()
}

#[test]
fn cancelling_an_edit_clears_the_stale_draft_without_leaving_the_composer() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("message-actions-cancel-edit")
        .telegram(
            TelegramScenario::new()
                .bootstrap(
                    account("Ada")
                        .with_chat(chat(10, "Rust"))
                        .with_draft(10, "stale draft"),
                )
                .expect_load_history(10, [sent_message(41, "old text")]),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    choose_message_action(&mut app, "Edit")?;
    app.press(key::ESCAPE)?;

    app.screen().composer().expect_focused()?;
    app.screen().composer().expect_text("")?;
    app.screen().message_text("stale draft").expect_absent()?;
    app.expect_saved_draft(10, "")?;
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
    choose_message_action(&mut app, "Delete")?;
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
fn message_selection_is_visible_and_survives_navigation_and_resize() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("message-actions-selection")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(
                    10,
                    [
                        sent_message(41, "first"),
                        sent_message(42, "second"),
                        sent_message(43, "third"),
                    ],
                ),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    choose_message_action(&mut app, "Select Message")?;
    app.press(key::UP)?;
    choose_message_action(&mut app, "Select Message")?;

    assert!(selected_message_is_visible(&app, "second"));
    assert!(selected_message_is_visible(&app, "third"));
    app.resize(120, 32)?;
    assert!(selected_message_is_visible(&app, "second"));
    assert!(selected_message_is_visible(&app, "third"));

    choose_message_action(&mut app, "Select Message")?;
    assert!(!selected_message_is_visible(&app, "second"));
    assert!(selected_message_is_visible(&app, "third"));
    app.press(key::ESCAPE)?;
    app.screen().composer().expect_focused()?;
    assert!(!selected_message_is_visible(&app, "third"));
    app.expect_no_unhandled_work()
}

#[test]
fn compatible_actions_apply_to_every_selected_message() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("message-actions-batch-delete")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(
                    10,
                    [
                        sent_message(41, "keep"),
                        sent_message(42, "remove second"),
                        sent_message(43, "remove third"),
                    ],
                )
                .expect_delete_messages(10, [42, 43]),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    choose_message_action(&mut app, "Select Message")?;
    app.press(key::UP)?;
    choose_message_action(&mut app, "Select Message")?;
    choose_message_action(&mut app, "Delete")?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Delete 2 Messages?"))
    );
    app.press(key::ENTER)?;

    app.screen().message(42).expect_absent()?;
    app.screen().message(43).expect_absent()?;
    app.expect_no_durable_message(10, 42)?;
    app.expect_no_durable_message(10, 43)?;
    assert!(!selected_message_is_visible(&app, "keep"));
    app.expect_no_unhandled_work()
}

fn selected_message_is_visible(app: &TestSystem, body: &str) -> bool {
    app.screen()
        .rows()
        .iter()
        .any(|row| row.contains("[✓]") && row.contains(body))
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
    choose_message_action(&mut app, "Forward")?;
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
    choose_message_action(&mut app, "Select Message")?;
    app.press(key::UP)?;
    choose_message_action(&mut app, "Select Message")?;
    choose_message_action(&mut app, "Forward")?;
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
    choose_message_action(&mut app, "React")?;
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

fn choose_message_action(app: &mut TestSystem, label: &str) -> Result<()> {
    app.type_text("a")?;
    let rows = app.screen().rows();
    let title = rows
        .iter()
        .position(|row| row.contains("Message Actions"))
        .expect("Message Actions popup should render");
    let target = rows
        .iter()
        .skip(title + 2)
        .position(|row| row.contains(label))
        .unwrap_or_else(|| panic!("Message Action {label:?} should render in {rows:?}"));
    for _ in 0..target {
        app.press(key::DOWN)?;
    }
    app.press(key::ENTER)
}
