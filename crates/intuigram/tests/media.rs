use intuigram_app::{MediaCard, MediaKind};
use test_harness::{Result, TelegramScenario, TestSystem, account, chat, incoming, key};

#[test]
fn unsupported_content_remains_visible_as_an_informative_media_card() -> Result<()> {
    let mut unsupported = incoming(40, "Lin", "[Unsupported content]");
    unsupported.details.media = Some(MediaCard {
        kind: MediaKind::Unsupported,
        title: "Unsupported Content".to_owned(),
        description: "This Telegram content is not supported by this build".to_owned(),
        details: Vec::new(),
        poll: None,
        remote_id: None,
    });
    let mut app = TestSystem::builder()
        .name("media-unsupported-content")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [unsupported]),
        )
        .start()?;

    app.press(key::ENTER)?;

    app.screen()
        .media_card("Unsupported Content")
        .expect_description("This Telegram content is not supported by this build")?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("Unsupported Content"))
    );
    app.expect_no_unhandled_work()
}

#[test]
fn uncaptioned_photo_is_rendered_inline_without_its_text_fallback() -> Result<()> {
    let mut photo = incoming(41, "Lin", "[photo.png] image/png");
    photo.details.media = Some(MediaCard {
        kind: MediaKind::Photo,
        title: "photo.png".to_owned(),
        description: "image/png".to_owned(),
        details: Vec::new(),
        poll: None,
        remote_id: Some("photo:41".to_owned()),
    });
    let mut app = TestSystem::builder()
        .name("media-inline-downloaded-photo")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [photo])
                .expect_media_preview(10, 41),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.choose_action("Download")?;

    let rows = app.screen().rows();
    assert!(rows.iter().any(|row| row.contains('▀')));
    assert!(rows.iter().all(|row| !row.contains("[photo.png]")));
    assert!(rows.iter().all(|row| !row.contains("image/png")));
    app.expect_no_unhandled_work()
}

#[test]
fn photo_preview_is_loaded_when_the_chat_opens() -> Result<()> {
    let mut photo = incoming(44, "Lin", "automatic preview");
    photo.details.media = Some(MediaCard {
        kind: MediaKind::Photo,
        title: "automatic.png".to_owned(),
        description: "image/png".to_owned(),
        details: Vec::new(),
        poll: None,
        remote_id: Some("photo:44".to_owned()),
    });
    let mut app = TestSystem::builder()
        .name("media-automatic-photo-preview")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [photo])
                .expect_media_preview(10, 44),
        )
        .start()?;

    app.press(key::ENTER)?;

    let rows = app.screen().rows();
    let image_row = rows
        .iter()
        .position(|row| row.contains('▀'))
        .expect("image should render");
    let caption_row = rows
        .iter()
        .position(|row| row.contains("automatic preview"))
        .expect("caption should render");
    assert!(caption_row > image_row);
    assert!(app.downloaded_paths().is_empty());
    app.expect_no_unhandled_work()
}

#[test]
fn inline_image_has_vertical_padding_inside_the_active_message_rule() -> Result<()> {
    let mut photo = incoming(45, "Lin", "padded preview");
    photo.details.media = Some(MediaCard {
        kind: MediaKind::Photo,
        title: "padded.png".to_owned(),
        description: "image/png".to_owned(),
        details: Vec::new(),
        poll: None,
        remote_id: Some("photo:45".to_owned()),
    });
    let mut app = TestSystem::builder()
        .name("media-inline-image-padding")
        .terminal(100, 40)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [photo])
                .expect_media_preview(10, 45),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    let rows = app.screen().rows();
    let image_rows = rows
        .iter()
        .enumerate()
        .filter_map(|(row, content)| content.contains('▀').then_some(row))
        .collect::<Vec<_>>();
    let first = *image_rows.first().expect("image should render");
    let last = *image_rows.last().expect("image should render");

    assert!(image_rows.len() >= 10);
    assert_eq!(app.screen().symbol_at(34, (first - 1) as u16), "│");
    assert_eq!(app.screen().symbol_at(34, (last + 1) as u16), "│");
    assert!(transcript_content(&rows[first - 1]).trim().is_empty());
    assert!(transcript_content(&rows[last + 1]).trim().is_empty());
    app.screen().message(45).expect_continuous_active_rule()?;
    app.expect_no_unhandled_work()
}

#[test]
fn downloaded_photos_keep_independent_inline_previews() -> Result<()> {
    let photos = [41, 42].map(|id| {
        let mut photo = incoming(id, "Lin", format!("photo {id}"));
        photo.details.media = Some(MediaCard {
            kind: MediaKind::Photo,
            title: format!("photo-{id}.png"),
            description: "image/png".to_owned(),
            details: Vec::new(),
            poll: None,
            remote_id: Some(format!("photo:{id}")),
        });
        photo
    });
    let mut app = TestSystem::builder()
        .name("media-independent-inline-previews")
        .terminal(100, 60)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, photos)
                .expect_media_preview(10, 42)
                .expect_media_preview(10, 41),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.choose_action("Download")?;
    app.press(key::ALT_UP)?;
    app.choose_action("Download")?;

    let preview_groups = app
        .screen()
        .rows()
        .iter()
        .fold((0, false), |(groups, previous), row| {
            let current = row.contains('▀');
            (groups + usize::from(current && !previous), current)
        })
        .0;
    assert_eq!(preview_groups, 2);
    app.expect_no_unhandled_work()
}

#[test]
fn loaded_preview_survives_redraw_scroll_and_chat_reselection() -> Result<()> {
    let mut photo = incoming(46, "Lin", "retained preview");
    photo.details.media = Some(MediaCard {
        kind: MediaKind::Sticker,
        title: "sticker.webp".to_owned(),
        description: "image/webp".to_owned(),
        details: Vec::new(),
        poll: None,
        remote_id: Some("sticker:46".to_owned()),
    });
    let fixture = account("Ada")
        .with_chat(chat(10, "Rust"))
        .with_chat(chat(20, "Design"))
        .with_history([photo.clone()]);
    let mut app = TestSystem::builder()
        .name("media-retained-inline-preview")
        .terminal(100, 40)
        .telegram(
            TelegramScenario::new()
                .bootstrap(fixture)
                .expect_media_preview(10, 46)
                .expect_load_history(20, [])
                .expect_load_history(10, [photo]),
        )
        .start()?;

    assert!(app.screen().rows().iter().any(|row| row.contains('▀')));
    app.resize(120, 42)?;
    app.press(key::DOWN)?;
    app.press(key::UP)?;
    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;

    let rows = app.screen().rows();
    assert!(rows.iter().any(|row| row.contains('▀')));
    assert!(rows.iter().all(|row| !row.contains("[sticker.webp]")));
    assert!(rows.iter().all(|row| !row.contains("image/webp")));
    app.expect_no_unhandled_work()
}

#[test]
fn failed_background_channel_refresh_does_not_block_an_image_preview() -> Result<()> {
    let mut photo = incoming(43, "Lin", "channel-safe preview");
    photo.details.media = Some(MediaCard {
        kind: MediaKind::Photo,
        title: "channel-safe.png".to_owned(),
        description: "image/png".to_owned(),
        details: Vec::new(),
        poll: None,
        remote_id: Some("photo:43".to_owned()),
    });
    let mut app = TestSystem::builder()
        .name("media-background-channel-invalid")
        .telegram(
            TelegramScenario::new()
                .bootstrap(
                    account("Ada")
                        .with_chat(chat(10, "Rust"))
                        .with_chat(chat(-1_001_195_461_650, "Unavailable Channel")),
                )
                .fail_load_history(-1_001_195_461_650, "CHANNEL_INVALID")
                .expect_load_history(10, [photo])
                .expect_media_preview(10, 43),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.choose_action("Download")?;

    assert!(app.screen().rows().iter().any(|row| row.contains('▀')));
    app.expect_no_unhandled_work()
}

fn transcript_content(row: &str) -> String {
    row.chars().skip(36).collect()
}
