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
    app.screen().action(Action::Paste).expect_available()?;
    for action in [
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
fn composer_attachment_tray_removes_only_the_active_item() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("composer-attachment-tray")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [])
                .hold_send_with_attachments("remaining", 10, "", ["two.pdf"]),
        )
        .start()?;

    app.press(key::ENTER)?;
    for path in ["/tmp/one.png", "/tmp/two.pdf"] {
        app.choose_action("Attach File")?;
        app.type_text(path)?;
        app.press(key::ENTER)?;
    }

    let rows = app.screen().rows();
    let first = rows
        .iter()
        .enumerate()
        .find_map(|(row, line)| line.find("one.png").map(|column| (row, column)))
        .expect("the first attachment must be listed");
    let second = rows
        .iter()
        .enumerate()
        .find_map(|(row, line)| line.find("two.pdf").map(|column| (row, column)))
        .expect("the second attachment must be listed");
    let input = rows
        .iter()
        .position(|row| row.contains("Type or paste a message"))
        .expect("the Composer input must be visible");
    let details = rows
        .iter()
        .position(|row| row.contains("Photo") && row.contains("File"))
        .expect("the attachment types must be listed");
    assert!(
        !rows.iter().any(|row| row.contains("attachment(s)")),
        "the attachment tray must not show a count heading"
    );
    assert_eq!(
        rows[first.0].find('│'),
        rows[input].find('│'),
        "the attachment and Composer accent rules must align"
    );
    assert_eq!(
        input,
        details.saturating_add(2),
        "one empty line must separate the attachment tray and Composer input"
    );
    assert!(
        !app.screen().background_is_default_at(
            u16::try_from(first.1).unwrap_or(u16::MAX),
            u16::try_from(first.0.saturating_sub(1)).unwrap_or(u16::MAX)
        ),
        "the attachment tray top padding must use the Composer surface"
    );
    assert_eq!(first.0, second.0);
    assert!(first.1 < second.1 && second.0 < input);
    assert!(
        rows[..input].iter().any(|row| row.contains('▀')),
        "the photo attachment must show a rendered preview"
    );
    assert!(
        rows[..input].iter().any(|row| row.contains("FILE")),
        "the file attachment must show a text fallback"
    );
    app.press(key::ALT_LEFT)?;
    app.press(key::REMOVE_ATTACHMENT)?;
    assert!(
        !app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("one.png"))
    );
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("two.pdf"))
    );

    app.press(key::ENTER)?;
    app.telegram().complete("remaining", sent_message(41, ""))?;
    app.expect_no_unhandled_work()
}

#[test]
fn composer_accepts_committed_ime_text_and_shifted_symbols() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("composer-committed-text")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, []),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.type_text("你好?")?;

    app.screen().composer().expect_text("你好?")?;
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
    app.press(key::CTRL_PASTE)?;

    app.screen().composer().expect_text("caption")?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("clipboard.png"))
    );
    app.press(key::ENTER)?;
    app.telegram()
        .complete("image", sent_message(41, "caption"))?;

    app.expect_no_unhandled_work()
}
