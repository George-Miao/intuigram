use test_harness::{Result, TelegramScenario, TestSystem, account, chat, incoming, key};

#[test]
fn default_view_separates_chats_and_messages_and_uses_a_three_line_folder_bar() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("layout-default-spacing")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(
                    account("Ada")
                        .with_chat(chat(10, "Rust"))
                        .with_chat(chat(11, "Telegram")),
                )
                .expect_load_history(11, [])
                .expect_load_history(
                    10,
                    [
                        incoming(40, "Lin", "first message"),
                        incoming(41, "Lin", "second message"),
                    ],
                ),
        )
        .start()?;

    app.press(key::ENTER)?;
    let rows = app.screen().rows();
    let rust = row_within(&rows, "Rust", 0, 30);
    let telegram = row_within(&rows, "Telegram", 0, 30);
    let first = row_within(&rows, "first message", 31, 100);
    let second = row_within(&rows, "second message", 31, 100);

    assert_eq!(telegram.saturating_sub(rust), 3);
    assert_eq!(second.saturating_sub(first), 2);
    let folder = rows
        .iter()
        .position(|row| row.contains("All"))
        .expect("folder strip should render");
    assert!(rows[folder - 1].trim().is_empty());
    assert!(rows[folder + 1].trim().is_empty());
    app.expect_no_unhandled_work()
}

#[test]
fn composer_is_one_continuous_bar_with_internal_padding() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("layout-composer-padding")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, []),
        )
        .start()?;

    app.press(key::ENTER)?;
    let rows = app.screen().rows();
    let placeholder = row_within(&rows, "Type or paste a message…", 31, 100);

    assert!(!row_segment(&rows, placeholder, 31, 100).contains("Draft"));
    assert!(
        row_segment(&rows, placeholder - 1, 31, 100)
            .trim()
            .is_empty()
    );
    assert!(
        row_segment(&rows, placeholder + 1, 31, 100)
            .trim()
            .is_empty()
    );
    assert_eq!(row_segment(&rows, placeholder, 31, 32), " ");
    assert_eq!(row_segment(&rows, placeholder, 99, 100), " ");
    app.expect_no_unhandled_work()
}

#[test]
fn chat_list_and_transcript_have_one_cell_internal_padding() -> Result<()> {
    let mut rust = chat(10, "Rust");
    rust.preview = "owned buffers keep terminal input responsive".to_owned();
    let mut app = TestSystem::builder()
        .name("layout-chat-title-alignment")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(rust))
                .expect_load_history(10, [incoming(40, "Lin", "hello")]),
        )
        .start()?;

    app.press(key::ENTER)?;
    let rows = app.screen().rows();
    let title_row = row_within(&rows, "Rust", 0, 30);
    let preview_row = row_within(&rows, "owned buffers", 0, 30);
    let message_row = row_within(&rows, "hello", 31, 100);

    assert_eq!(row_segment(&rows, title_row, 0, 7), " │ [RU]");
    assert_eq!(row_segment(&rows, preview_row, 28, 30), "  ");
    assert!(row_segment(&rows, 4, 31, 100).trim().is_empty());
    assert_eq!(row_segment(&rows, message_row, 31, 35), "   h");
    let folder = rows
        .iter()
        .position(|row| row.contains("All"))
        .expect("folder strip should render");
    let action = rows
        .iter()
        .position(|row| row.contains("Send"))
        .expect("action bar should render");
    let status = rows
        .iter()
        .position(|row| row.contains("connected"))
        .expect("status bar should render");
    assert!(rows[folder].starts_with(' '));
    assert!(rows[action].starts_with(' '));
    assert!(rows[status].starts_with(' '));
    app.expect_no_unhandled_work()
}

#[test]
fn only_the_focused_block_uses_a_surface_background() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("layout-focused-surface")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [incoming(40, "Lin", "hello")]),
        )
        .start()?;

    let canvas = app.screen().background_at(30, 6);
    assert_ne!(app.screen().background_at(5, 6), canvas);
    assert_eq!(app.screen().background_at(40, 6), canvas);

    app.press(key::ENTER)?;
    let rows = app.screen().rows();
    let composer = row_within(&rows, "Type or paste a message…", 31, 100);

    assert_eq!(app.screen().background_at(5, 6), canvas);
    assert_eq!(app.screen().background_at(40, 6), canvas);
    assert_ne!(app.screen().background_at(40, composer as u16), canvas);
    app.expect_no_unhandled_work()
}

#[test]
fn quoted_messages_keep_their_content_inside_one_trailing_blank_row() -> Result<()> {
    let mut quoted = incoming(41, "Lin", "quoted reply");
    quoted.reply_to = Some(intuigram_app::MessageId(40));
    let mut app = TestSystem::builder()
        .name("layout-quoted-message-spacing")
        .terminal(100, 28)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(
                    10,
                    [
                        incoming(40, "Lin", "original"),
                        quoted,
                        incoming(42, "Ada", "following message"),
                    ],
                ),
        )
        .start()?;

    app.press(key::ENTER)?;
    let rows = app.screen().rows();
    let quote = row_within(&rows, "Lin: original", 31, 100);
    let reply = row_within(&rows, "quoted reply", 31, 100);
    let following = row_within(&rows, "following message", 31, 100);

    assert!(quote < reply);
    assert!(row_segment(&rows, reply + 1, 31, 100).trim().is_empty());
    assert!(following >= reply + 2);
    app.expect_no_unhandled_work()
}

#[test]
fn headers_have_vertical_padding_and_an_active_chat_status_row() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("layout-padded-headers")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, []),
        )
        .start()?;

    app.press(key::ENTER)?;
    let rows = app.screen().rows();

    assert!(row_segment(&rows, 0, 0, 30).trim().is_empty());
    assert!(row_segment(&rows, 0, 31, 100).trim().is_empty());
    assert!(row_segment(&rows, 1, 0, 30).contains("Chats"));
    assert!(row_segment(&rows, 1, 31, 100).contains("Rust"));
    assert!(row_segment(&rows, 2, 31, 100).contains("last seen recently"));
    assert!(row_segment(&rows, 3, 31, 100).trim().is_empty());
    app.expect_no_unhandled_work()
}

#[test]
fn resize_preserves_message_draft_and_interaction_state_across_layout_classes() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("layout-resize-preserves-state")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(
                    10,
                    [
                        incoming(40, "Lin", "first message"),
                        incoming(41, "Lin", "second message"),
                    ],
                ),
        )
        .start()?;
    app.press(key::ENTER)?;
    app.type_text("durable draft")?;
    app.press(key::ALT_UP)?;

    app.resize(70, 24)?;
    app.screen().message(41).expect_active()?;
    app.screen().composer().expect_text("durable draft")?;
    app.screen().folder("All").expect_active()?;

    app.resize(160, 32)?;
    app.screen().message(41).expect_active()?;
    app.screen().composer().expect_text("durable draft")?;
    app.screen().chat("Rust").expect_active()?;
    app.screen().folder("All").expect_active()?;
    app.expect_no_unhandled_work()
}

fn row_within(rows: &[String], text: &str, start: usize, end: usize) -> usize {
    rows.iter()
        .position(|row| {
            row.chars()
                .skip(start)
                .take(end.saturating_sub(start))
                .collect::<String>()
                .contains(text)
        })
        .unwrap_or_else(|| panic!("{text:?} should be rendered between columns {start} and {end}"))
}

fn row_segment(rows: &[String], row: usize, start: usize, end: usize) -> String {
    rows[row]
        .chars()
        .chain(std::iter::repeat(' '))
        .skip(start)
        .take(end.saturating_sub(start))
        .collect()
}
