use intuigram_lib::{MediaCard, MediaKind};
use test_harness::{Result, TelegramScenario, TestSystem, account, chat, incoming, key};

#[test]
fn loaded_photo_opens_larger_by_click_and_action() -> Result<()> {
    let mut photo = incoming(46, "Lin", "popup preview");
    photo.details.media = Some(MediaCard {
        kind: MediaKind::Photo,
        title: "popup.png".to_owned(),
        description: "image/png".to_owned(),
        details: Vec::new(),
        poll: None,
        specialized: None,
        remote_id: Some("photo:46".to_owned()),
    });
    let mut app = TestSystem::builder()
        .name("media-image-popup")
        .terminal(100, 40)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [photo])
                .expect_media_preview(10, 46),
        )
        .start()?;
    app.press(key::ENTER)?;
    let inline_rows = image_row_count(&app);

    app.click_media("popup.png")?;
    assert!(image_row_count(&app) > inline_rows);
    app.press(key::ESCAPE)?;

    app.press(key::ALT_UP)?;
    app.choose_action("View Image")?;
    assert!(image_row_count(&app) > inline_rows);
    app.press(key::ESCAPE)?;
    app.expect_no_unhandled_work()
}

fn image_row_count(app: &TestSystem) -> usize {
    app.screen()
        .rows()
        .iter()
        .filter(|row| row.contains('▀'))
        .count()
}
