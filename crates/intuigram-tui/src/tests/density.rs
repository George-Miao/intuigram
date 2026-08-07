use super::*;

#[test]
fn compact_view_preserves_the_original_dense_layout() {
    let mut current = view(Vec::new());
    current.folders = vec![FolderView {
        id: 0,
        title: "All".to_owned(),
        unread: 0,
    }];
    current.chats = vec![
        ChatView {
            id: ChatId(10),
            title: "First".to_owned(),
            preview: String::new(),
            unread: 0,
            pinned: false,
            kind: ChatKind::Private,
            folders: vec![0],
        },
        ChatView {
            id: ChatId(11),
            title: "Second".to_owned(),
            preview: String::new(),
            unread: 0,
            pinned: false,
            kind: ChatKind::Private,
            folders: vec![0],
        },
    ];
    current.active_chat = Some(0);
    current.messages = vec![message(1, "first message"), message(2, "second message")];

    let backend = TestBackend::new(100, 24);
    let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
    terminal
        .draw(|frame| {
            render_with_mode(
                frame,
                &current,
                &EffectiveKeymap::defaults(),
                ViewMode::Compact,
            );
        })
        .expect("compact view should render");
    let buffer = terminal.backend().buffer();

    assert_eq!(buffer[(4, 2)].symbol(), "F");
    assert_eq!(buffer[(4, 4)].symbol(), "S");
    assert_eq!(buffer[(33, 3)].symbol(), "f");
    assert_eq!(buffer[(33, 5)].symbol(), "s");
    assert_eq!(buffer[(2, 21)].symbol(), "A");
}

fn message(id: i64, body: &str) -> MessageView {
    MessageView {
        id: MessageId(id),
        sender: "Lin".to_owned(),
        body: body.to_owned(),
        timestamp: "12:00".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: None,
        details: MessageDetails::default(),
    }
}
