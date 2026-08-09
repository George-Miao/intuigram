#[test]
fn composer_focus_renders_the_terminal_cursor_after_the_text() {
    let mut current = active_chat_view();
    current.composer.text = "hi".to_owned();
    current.composer.cursor = current.composer.text.len();

    let mut terminal = terminal();
    terminal
        .draw(|frame| render(frame, &current, &EffectiveKeymap::defaults()))
        .expect("Composer should render");

    assert!(terminal.backend().cursor_visible());
    assert_eq!(terminal.backend().cursor_position(), (38, 20).into());
}

#[test]
fn multiline_composer_grows_to_a_cap_and_scrolls_with_the_cursor() {
    let mut current = active_chat_view();
    current.composer.text = (1..=10)
        .map(|line| format!("line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    current.composer.cursor = current.composer.text.len();

    let mut terminal = terminal();
    terminal
        .draw(|frame| render(frame, &current, &EffectiveKeymap::defaults()))
        .expect("multiline Composer should render");
    let buffer = terminal.backend().buffer();

    assert_eq!(buffer[(40, 13)].bg, Color::Rgb(230, 226, 204));
    assert_eq!(terminal.backend().cursor_position(), (43, 20).into());
    assert!(row_text(buffer, 20).contains("line 10"));
    assert!(!row_text(buffer, 14).contains("line 1"));

    current.composer.text.clear();
    current.composer.cursor = 0;
    terminal
        .draw(|frame| render(frame, &current, &EffectiveKeymap::defaults()))
        .expect("empty Composer should shrink");
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(40, 18)].bg, Color::Reset);
    assert_eq!(buffer[(40, 19)].bg, Color::Rgb(230, 226, 204));
}

#[test]
fn composer_height_accounts_for_soft_wrapping() {
    let mut current = active_chat_view();
    current.composer.text = "x".repeat(130);
    current.composer.cursor = current.composer.text.len();

    let mut terminal = terminal();
    terminal
        .draw(|frame| render(frame, &current, &EffectiveKeymap::defaults()))
        .expect("soft-wrapped Composer should render");
    let buffer = terminal.backend().buffer();

    assert_eq!(buffer[(40, 16)].bg, Color::Reset);
    assert_eq!(buffer[(40, 17)].bg, Color::Rgb(230, 226, 204));
    assert_eq!(terminal.backend().cursor_position().y, 20);
}

#[test]
fn transcript_preserves_explicit_message_line_breaks() {
    let mut current = active_chat_view();
    current.messages = vec![MessageView {
        id: MessageId(1),
        sender: "Lin".to_owned(),
        body: "first line\n\nthird line".to_owned(),
        timestamp: "12:00".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: None,
        details: MessageDetails::default(),
    }];

    let mut terminal = terminal();
    terminal
        .draw(|frame| render(frame, &current, &EffectiveKeymap::defaults()))
        .expect("multiline Message should render");
    let buffer = terminal.backend().buffer();
    let first = (0..24)
        .find(|row| row_text(buffer, *row).contains("first line"))
        .expect("first line should render");

    assert!(row_text(buffer, first + 1).trim().is_empty());
    assert!(row_text(buffer, first + 2).contains("third line"));
}

fn active_chat_view() -> View {
    let mut current = view(Vec::new());
    current.chats = vec![ChatView {
        id: ChatId(10),
        title: "Intuigram".to_owned(),
        preview: String::new(),
        preview_sender: None,
        preview_sender_peer: None,
        preview_timestamp: String::new(),
        status: String::new(),
        unread: 0,
        pinned: false,
        can_pin_messages: true,
        has_topics: false,
        kind: ChatKind::Private,
        folders: vec![0],
    }];
    current.active_chat = Some(0);
    current.focus = Focus::Composer;
    current
}

fn terminal() -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(100, 28)).expect("test terminal should initialize")
}

fn row_text(buffer: &ratatui::buffer::Buffer, row: u16) -> String {
    (0..buffer.area.width)
        .map(|column| buffer[(column, row)].symbol())
        .collect()
}

use super::*;
