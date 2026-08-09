use super::*;

#[test]
fn fresh_loading_is_centered_and_incremental_loading_keeps_cached_messages_visible() {
    let mut current = active_chat();
    current.chat_loading = ChatLoadingState::Fresh;
    let fresh = render_rows(&current);
    let brand_row = fresh
        .iter()
        .position(|row| row.contains("INTUIGRAM"))
        .expect("fresh loading brand should render");
    let progress_row = fresh
        .iter()
        .position(|row| row.contains("[>"))
        .expect("paper-plane progress should render");
    let status_row = fresh
        .iter()
        .position(|row| row.contains("syncing chat"))
        .expect("fresh loading status should render");
    assert_eq!(progress_row, brand_row + 1);
    assert_eq!(status_row, progress_row + 1);
    assert!((7..=11).contains(&brand_row));
    assert!((61..=65).contains(&fresh[brand_row].find("INTUIGRAM").unwrap()));
    assert!(!fresh[2].contains("updating"));

    current.animation_frame = 1;
    let advanced = render_rows(&current);
    assert!(advanced.iter().any(|row| row.contains("[->")));

    current.chat_loading = ChatLoadingState::Updating;
    current.messages = vec![message("cached message")];
    let updating = render_rows(&current);
    assert!(updating[2].contains("updating"));
    assert!(updating.iter().any(|row| row.contains("cached message")));
}

#[test]
fn fresh_loading_compacts_without_disappearing_in_reduced_space() {
    let mut current = active_chat();
    current.chat_loading = ChatLoadingState::Fresh;

    let short = render_rows_at(&current, 80, 18);
    assert!(short.iter().any(|row| row.contains("syncing chat")));
    let narrow = render_rows_at(&current, 40, 18);
    assert!(narrow.iter().any(|row| row.contains("loading")));
}

fn active_chat() -> View {
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
        kind: ChatKind::Private,
        folders: vec![0],
    }];
    current.active_chat = Some(0);
    current.focus = Focus::Composer;
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
    render_rows_at(current, 100, 28)
}

fn render_rows_at(current: &View, width: u16, height: u16) -> Vec<String> {
    let mut terminal =
        Terminal::new(TestBackend::new(width, height)).expect("test terminal should initialize");
    terminal
        .draw(|frame| render(frame, current, &EffectiveKeymap::defaults()))
        .expect("loading state should render");
    (0..height)
        .map(|row| {
            (0..width)
                .map(|column| terminal.backend().buffer()[(column, row)].symbol())
                .collect::<String>()
        })
        .collect()
}
