use super::*;

#[test]
fn fresh_loading_is_centered_and_incremental_loading_keeps_cached_messages_visible() {
    let mut current = active_chat();
    current.chat_loading = ChatLoadingState::Fresh;
    let fresh = render_rows(&current);
    let loading_row = fresh
        .iter()
        .position(|row| row.contains("loading"))
        .expect("fresh loading text should render");
    let loading_column = fresh[loading_row]
        .find("loading")
        .expect("loading column should exist");
    assert!((8..=12).contains(&loading_row));
    assert!((60..=66).contains(&loading_column));
    assert!(!fresh[2].contains("updating"));

    current.chat_loading = ChatLoadingState::Updating;
    current.messages = vec![message("cached message")];
    let updating = render_rows(&current);
    assert!(updating[2].contains("updating"));
    assert!(updating.iter().any(|row| row.contains("cached message")));
}

fn active_chat() -> View {
    let mut current = view(Vec::new());
    current.chats = vec![ChatView {
        id: ChatId(10),
        title: "Intuigram".to_owned(),
        preview: String::new(),
        status: String::new(),
        unread: 0,
        pinned: false,
        can_pin_messages: true,
        kind: ChatKind::Private,
        folders: vec![0],
    }];
    current.active_chat = Some(0);
    current
}

fn message(body: &str) -> MessageView {
    MessageView {
        id: MessageId(40),
        sender: "Lin".to_owned(),
        body: body.to_owned(),
        timestamp: "12:00".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: None,
        details: MessageDetails::default(),
    }
}

fn render_rows(current: &View) -> Vec<String> {
    let mut terminal =
        Terminal::new(TestBackend::new(100, 24)).expect("test terminal should initialize");
    terminal
        .draw(|frame| render(frame, current, &EffectiveKeymap::defaults()))
        .expect("loading state should render");
    (0..24)
        .map(|row| {
            (0..100)
                .map(|column| terminal.backend().buffer()[(column, row)].symbol())
                .collect::<String>()
        })
        .collect()
}
