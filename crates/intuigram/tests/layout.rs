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
    assert_eq!(second.saturating_sub(first), 3);
    assert!(rows[19].trim().is_empty());
    assert!(rows[20].contains("All"));
    assert!(rows[21].trim().is_empty());
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
    let draft = row_within(&rows, "Draft", 31, 100);
    let placeholder = row_within(&rows, "Type or paste a message…", 31, 100);

    assert_eq!(draft, placeholder);
    assert!(row_segment(&rows, draft - 1, 31, 100).trim().is_empty());
    assert!(row_segment(&rows, draft + 1, 31, 100).trim().is_empty());
    assert_eq!(row_segment(&rows, draft, 31, 32), " ");
    assert_eq!(row_segment(&rows, draft, 99, 100), " ");
    app.expect_no_unhandled_work()
}

#[test]
fn chat_list_and_transcript_have_one_cell_internal_padding() -> Result<()> {
    let mut rust = chat(10, "Rust");
    rust.preview = "owned buffers".to_owned();
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
    let message_row = row_within(&rows, "hello", 31, 100);

    assert_eq!(row_segment(&rows, title_row, 0, 7), " │ Rust");
    assert!(row_segment(&rows, 2, 31, 100).trim().is_empty());
    assert_eq!(row_segment(&rows, message_row, 31, 35), "   h");
    assert!(rows[20].starts_with(' '));
    assert!(rows[22].starts_with(' '));
    assert!(rows[23].starts_with(' '));
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
