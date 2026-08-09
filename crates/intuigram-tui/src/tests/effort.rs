#[test]
fn pending_work_uses_a_moving_highlight_and_stops_when_complete() {
    let mut current = view(Vec::new());
    current.connection = ConnectionState::Connecting;
    current.chat_loading = ChatLoadingState::Updating;
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
    current.messages = vec![MessageView {
        id: MessageId(-1),
        sender: "You".to_owned(),
        body: "pending".to_owned(),
        timestamp: "now".to_owned(),
        direction: MessageDirection::Outgoing,
        delivery: DeliveryState::Pending,
        reply_to: None,
        details: MessageDetails::default(),
    }];

    let mut terminal =
        Terminal::new(TestBackend::new(100, 28)).expect("test terminal should initialize");
    terminal
        .draw(|frame| render(frame, &current, &EffectiveKeymap::defaults()))
        .expect("first effort frame should render");
    let first = highlighted_columns(terminal.backend().buffer(), 26);
    let rendered = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(rendered.contains("connecting"));
    assert!(rendered.contains("sending"));
    assert!(rendered.contains("synchronizing"));

    current.animation_frame = 1;
    terminal
        .draw(|frame| render(frame, &current, &EffectiveKeymap::defaults()))
        .expect("next effort frame should render");
    let second = highlighted_columns(terminal.backend().buffer(), 26);
    assert_ne!(first, second);

    current.connection = ConnectionState::Connected;
    current.chat_loading = ChatLoadingState::Idle;
    current.messages[0].delivery = DeliveryState::Sent;
    terminal
        .draw(|frame| render(frame, &current, &EffectiveKeymap::defaults()))
        .expect("completed work should render");
    let rendered = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(!rendered.contains("sending"));
    assert!(rendered.contains("connected"));
}

fn highlighted_columns(buffer: &ratatui::buffer::Buffer, row: u16) -> Vec<u16> {
    (0..buffer.area.width)
        .filter(|column| buffer[(*column, row)].fg == Color::Rgb(141, 161, 1))
        .collect()
}

use super::*;
