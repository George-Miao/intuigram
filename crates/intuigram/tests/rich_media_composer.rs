use test_harness::{Result, TelegramScenario, TestSystem, account, chat, key};

fn composer(name: &str) -> Result<TestSystem> {
    let mut app = TestSystem::builder()
        .name(name)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, []),
        )
        .start()?;
    app.press(key::ENTER)?;
    app.choose_action("Media & Contacts")?;
    Ok(app)
}

#[test]
fn saved_media_can_be_browsed_and_sent_from_the_composer() -> Result<()> {
    let mut app = composer("rich-media-library")?;

    app.press(key::ENTER)?;
    assert!(app.screen().rows().iter().any(|row| row.contains("wave")));
    app.press(key::ENTER)?;

    assert!(app.screen().rows().iter().any(|row| row.contains("wave")));
    app.expect_no_unhandled_work()
}

#[test]
fn an_exact_local_path_is_sent_with_the_selected_media_type() -> Result<()> {
    let mut app = composer("rich-media-file")?;

    for _ in 0..3 {
        app.press(key::DOWN)?;
    }
    app.press(key::ENTER)?;
    app.type_text("/tmp/a.gif")?;
    app.press(key::DOWN)?;
    app.press(key::SPACE)?;
    app.press(key::ENTER)?;

    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("[Animation] a.gif"))
    );
    app.expect_no_unhandled_work()
}

#[test]
fn voice_and_video_note_recordings_are_available() -> Result<()> {
    for (name, menu_row, label) in [
        ("rich-media-voice", 4, "[Voice]"),
        ("rich-media-video-note", 5, "[VideoNote]"),
    ] {
        let mut app = composer(name)?;
        for _ in 0..menu_row {
            app.press(key::DOWN)?;
        }
        app.press(key::ENTER)?;
        app.type_text("3")?;
        app.press(key::DOWN)?;
        app.type_text("default")?;
        app.press(key::ENTER)?;
        assert!(app.screen().rows().iter().any(|row| row.contains(label)));
        app.expect_no_unhandled_work()?;
    }
    Ok(())
}

#[test]
fn a_contact_card_uses_all_composer_fields() -> Result<()> {
    let mut app = composer("rich-media-contact")?;

    for _ in 0..6 {
        app.press(key::DOWN)?;
    }
    app.press(key::ENTER)?;
    app.type_text("+123")?;
    app.press(key::DOWN)?;
    app.type_text("Ada")?;
    app.press(key::DOWN)?;
    app.type_text("Lovelace")?;
    app.press(key::ENTER)?;

    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("[Contact] Ada Lovelace"))
    );
    app.expect_no_unhandled_work()
}
