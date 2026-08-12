use super::*;

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
    assert_eq!(second.saturating_sub(first), 1);
    let folder = rows
        .iter()
        .position(|row| row.contains("All"))
        .expect("folder strip should render");
    assert!(rows[folder - 1].trim().is_empty());
    assert!(rows[folder + 1].trim().is_empty());
    app.expect_no_unhandled_work()
}
