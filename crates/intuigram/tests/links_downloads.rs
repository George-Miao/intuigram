use intuigram_app::{Action, MediaCard, MediaKind, TextEntity, TextEntityKind};
use test_harness::{Result, TelegramScenario, TestSystem, account, chat, incoming, key};

#[test]
fn disguised_links_are_confirmed_and_launchable_downloads_are_only_revealed() -> Result<()> {
    let mut message = incoming(40, "Lin", "https://example.com");
    message.details.entities = vec![TextEntity {
        offset: 0,
        length: 19,
        kind: TextEntityKind::TextUrl {
            url: "https://evil.example/login".to_owned(),
        },
    }];
    message.details.media = Some(MediaCard {
        kind: MediaKind::File,
        title: "installer.sh".to_owned(),
        description: "application/x-shellscript".to_owned(),
        details: Vec::new(),
        poll: None,
        remote_id: Some("document:40".to_owned()),
    });
    let mut app = TestSystem::builder()
        .name("links-and-downloads")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [message]),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.choose_action("Open Link")?;
    app.screen()
        .action(Action::ConfirmOpenLink)
        .expect_available()?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("https://evil.example/login"))
    );
    app.press(key::ENTER)?;
    assert_eq!(app.opened_links(), &["https://evil.example/login"]);

    app.choose_action("Download")?;
    app.screen()
        .action(Action::OpenDownload)
        .expect_unavailable()?;
    assert_eq!(app.downloaded_paths().len(), 1);
    app.choose_action("Open Download")?;
    assert_eq!(app.opened_downloads().len(), 1);
    assert!(app.opened_downloads()[0].1);
    app.expect_no_unhandled_work()
}
