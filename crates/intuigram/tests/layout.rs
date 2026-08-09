use intuigram_app::ChatKind;
use test_harness::{Result, TelegramScenario, TestSystem, account, chat, incoming, key};

#[path = "layout/avatars.rs"]
mod avatars;
#[path = "layout/density.rs"]
mod density;

#[test]
fn long_transcript_messages_wrap_inside_the_active_chat() -> Result<()> {
    let body = format!(
        "A long Telegram message begins here {} tail-marker remains visible.",
        "x".repeat(100)
    );
    let mut app = TestSystem::builder()
        .name("layout-long-message-wrap")
        .terminal(200, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [incoming(40, "Lin", body)]),
        )
        .start()?;

    app.press(key::ENTER)?;
    let rows = app.screen().rows();
    let beginning = row_within(&rows, "A long Telegram message", 41, 200);
    let tail = row_within(&rows, "tail-marker", 41, 200);

    assert!(tail > beginning);
    app.expect_no_unhandled_work()
}

#[test]
fn metadata_stays_at_transcript_right_edge_when_content_is_capped() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("layout-message-content-width")
        .terminal(200, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [incoming(40, "Lin", "short message")]),
        )
        .start()?;

    app.press(key::ENTER)?;
    let rows = app.screen().rows();
    let metadata = row_within(&rows, "12:00 · ✓✓", 41, 200);

    assert_eq!(app.screen().symbol_at(198, metadata as u16), "✓");
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
fn chat_list_uses_dense_left_edge_while_transcript_and_chrome_stay_padded() -> Result<()> {
    let mut rust = chat(10, "abcdefghijklmnopqrstuvwxy");
    rust.preview = "owned buffers keep terminal input responsive".to_owned();
    rust.unread = 83;
    let mut app = TestSystem::builder()
        .name("layout-chat-title-alignment")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(rust))
                .expect_load_history(10, [incoming(40, "Lin", "hello")])
                .expect_read_history(10, 40),
        )
        .start()?;

    let chat_rows = app.screen().rows();
    let title_row = row_within(&chat_rows, "abcdefghijkl", 0, 32);
    let preview_row = row_within(&chat_rows, "owned buffers", 0, 32);
    assert_eq!(row_segment(&chat_rows, title_row, 0, 6), "│ [AB]");
    assert_eq!(row_segment(&chat_rows, title_row, 24, 27), "...");
    assert_eq!(row_segment(&chat_rows, title_row, 28, 30), "83");
    assert_eq!(row_segment(&chat_rows, preview_row, 27, 30), "...");
    assert_eq!(row_segment(&chat_rows, preview_row, 30, 32), "  ");

    app.press(key::ENTER)?;
    let rows = app.screen().rows();
    let message_row = row_within(&rows, "hello", 33, 100);

    assert!(row_segment(&rows, 4, 33, 100).trim().is_empty());
    assert_eq!(row_segment(&rows, message_row, 33, 42), "        h");
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
fn bottom_chrome_combines_status_and_context_actions() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("layout-combined-bottom-chrome")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, []),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ESCAPE)?;
    let rows = app.screen().rows();
    let bottom = rows
        .iter()
        .find(|row| row.contains("connected"))
        .expect("combined bottom chrome should show the idle status");

    assert!(bottom.contains("Enter Open"), "bottom chrome: {bottom:?}");
    assert!(!bottom.contains("Ada"));
    assert!(!bottom.contains("Chats"));
    app.expect_no_unhandled_work()
}

#[test]
fn popups_have_one_cell_padding_and_clip_safely_on_narrow_terminals() -> Result<()> {
    let mut app = TestSystem::builder()
        .name("layout-popup-padding")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [test_harness::sent_message(41, "popup target")]),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.type_text("a")?;
    let rows = app.screen().rows();
    let title_row = rows
        .iter()
        .position(|row| row.contains("Message Actions"))
        .expect("Message Actions popup should render");

    assert_eq!(row_segment(&rows, title_row, 29, 45), " Message Actions");
    assert!(row_segment(&rows, title_row - 1, 29, 71).trim().is_empty());

    app.resize(12, 5)?;
    assert_eq!(app.screen().rows().len(), 5);
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

    assert!(app.screen().background_is_default_at(32, 6));
    assert!(!app.screen().background_is_default_at(5, 6));
    assert!(app.screen().background_is_default_at(40, 6));

    app.press(key::ENTER)?;
    let rows = app.screen().rows();
    let composer = row_within(&rows, "Type or paste a message…", 33, 100);

    assert!(app.screen().background_is_default_at(5, 6));
    assert!(app.screen().background_is_default_at(40, 6));
    assert!(!app.screen().background_is_default_at(40, composer as u16));
    assert!(app.screen().background_is_default_at(32, 23));
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
    assert!(row_segment(&rows, quote - 1, 31, 100).trim().is_empty());
    assert!(row_segment(&rows, quote + 1, 31, 100).trim().is_empty());
    assert_eq!(reply, quote + 2);
    assert!(row_segment(&rows, reply + 1, 31, 100).trim().is_empty());
    assert!(following >= reply + 2);
    app.expect_no_unhandled_work()
}

#[test]
fn active_message_rule_spans_forward_and_quote_padding() -> Result<()> {
    let mut quoted = incoming(41, "Lin", "quoted forward");
    quoted.reply_to = Some(intuigram_app::MessageId(40));
    quoted.details.forwarded_from = Some("Runtime News".to_owned());
    let mut app = TestSystem::builder()
        .name("layout-continuous-active-message-rule")
        .terminal(100, 28)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, [incoming(40, "Lin", "original"), quoted]),
        )
        .start()?;

    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;

    app.screen().message(41).expect_active()?;
    app.screen().message(41).expect_continuous_active_rule()?;
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
fn opening_a_group_replaces_generic_metadata_with_member_presence() -> Result<()> {
    let mut group = chat(-1_000_000_000_010, "compio-rs");
    group.kind = ChatKind::Supergroup;
    group.status = "group".to_owned();
    let mut app = TestSystem::builder()
        .name("layout-group-presence-metadata")
        .terminal(100, 24)
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(group))
                .expect_load_history_with_status(-1_000_000_000_010, "240 members, 31 online", []),
        )
        .start()?;

    app.press(key::ENTER)?;
    assert!(
        app.screen()
            .rows()
            .iter()
            .any(|row| row.contains("240 members, 31 online"))
    );
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
