use intuigram_lib::Action;
use test_harness::{Result, TelegramScenario, TestSystem, account, chat, key, sent_message};

#[test]
fn composer_creation_actions_are_grouped_without_consuming_plain_text() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("composer-actions-popup")
        .terminal(100, 30)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, []),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.type_text("a")?;
    app.screen().composer().expect_text("a")?;
    app.press(key::BACKSPACE)?;
    app.screen()
        .action(Action::OpenActions)
        .expect_available()?;
    for action in [
        Action::Paste,
        Action::Attach,
        Action::OpenRichMedia,
        Action::OpenScheduled,
        Action::CreatePoll,
    ] {
        app.screen().action(action).expect_unavailable()?;
    }

    app.press(key::ALT_ACTIONS)?;
    let popup = app.screen().rows().join("\n");
    for label in [
        "Composer Actions",
        "Paste",
        "Attach File",
        "Media & Contacts",
        "Scheduled Messages",
        "Create Poll",
    ] {
        assert!(popup.contains(label), "missing {label:?} in {popup:?}");
    }

    for _ in 0..4 {
        app.press(key::DOWN)?;
    }
    app.press(key::ENTER)?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Poll · question first"))
    );
    app.expect_no_unhandled_work()
}

#[test]
fn attachment_action_falls_back_to_the_built_in_path_field() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("composer-attachment-path-fallback")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, []),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.choose_action("Attach File")?;

    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Attach local file"))
    );
    app.expect_no_unhandled_work()
}

#[test]
fn native_clipboard_image_preserves_caption_and_reaches_send_adapter() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("composer-native-image-paste")
        .clipboard_image()
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [])
                .hold_send_with_attachments("image", 10, "caption", ["clipboard.png"]),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.type_text("caption")?;
    app.choose_action("Paste")?;

    app.screen().composer().expect_text("caption")?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("1 attachment(s)"))
    );
    app.press(key::ENTER)?;
    app.telegram()
        .complete("image", sent_message(41, "caption"))?;

    app.expect_no_unhandled_work()
}
