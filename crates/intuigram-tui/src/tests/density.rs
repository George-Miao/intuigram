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
            status: String::new(),
            unread: 0,
            pinned: false,
            can_pin_messages: true,
            kind: ChatKind::Private,
            folders: vec![0],
        },
        ChatView {
            id: ChatId(11),
            title: "Second".to_owned(),
            preview: String::new(),
            status: String::new(),
            unread: 0,
            pinned: false,
            can_pin_messages: true,
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

    assert_eq!(buffer[(7, 2)].symbol(), "F");
    assert_eq!(buffer[(7, 4)].symbol(), "S");
    assert_eq!(buffer[(33, 3)].symbol(), "f");
    assert_eq!(buffer[(33, 5)].symbol(), "s");
    assert_eq!(buffer[(2, 21)].symbol(), "A");
}

#[test]
fn narrow_layout_projects_the_current_hierarchy_level() {
    let current = responsive_view();
    let chats = render_test_frame(&current, 70, 24);

    assert!(
        chats
            .semantics
            .iter()
            .any(|node| node.role == SemanticRole::Chat)
    );
    assert!(
        !chats
            .semantics
            .iter()
            .any(|node| matches!(node.role, SemanticRole::Message | SemanticRole::Composer))
    );

    let mut active_chat = current.clone();
    active_chat.focus = Focus::Transcript;
    active_chat.active_message = Some(1);
    active_chat.transcript_anchor = Some(1);
    let transcript = render_test_frame(&active_chat, 70, 24);

    assert!(
        !transcript
            .semantics
            .iter()
            .any(|node| node.role == SemanticRole::Chat)
    );
    assert!(transcript.semantics.iter().any(|node| {
        node.role == SemanticRole::Message
            && node.domain_id == Some(2)
            && node.active
            && node.focused
    }));
    assert!(
        transcript
            .semantics
            .iter()
            .any(|node| node.role == SemanticRole::Composer)
    );
}

#[test]
fn normal_and_wide_layouts_keep_both_columns_with_adaptive_proportions() {
    let current = responsive_view();
    let normal = render_test_frame(&current, 100, 24);
    let wide = render_test_frame(&current, 160, 30);
    let normal_chat = normal
        .semantics
        .iter()
        .find(|node| node.role == SemanticRole::Chat)
        .expect("normal layout should show Chats");
    let normal_message = normal
        .semantics
        .iter()
        .find(|node| node.role == SemanticRole::Message)
        .expect("normal layout should show Messages");
    let wide_chat = wide
        .semantics
        .iter()
        .find(|node| node.role == SemanticRole::Chat)
        .expect("wide layout should show Chats");
    let wide_message = wide
        .semantics
        .iter()
        .find(|node| node.role == SemanticRole::Message)
        .expect("wide layout should show Messages");

    assert_eq!(normal_chat.bounds.width, 28);
    assert_eq!(normal_message.bounds.x, 32);
    assert_eq!(wide_chat.bounds.width, 36);
    assert_eq!(wide_message.bounds.x, 40);
}

fn responsive_view() -> View {
    let mut current = view(Vec::new());
    current.folders = vec![FolderView {
        id: 0,
        title: "All".to_owned(),
        unread: 0,
    }];
    current.chats = vec![ChatView {
        id: ChatId(10),
        title: "First".to_owned(),
        preview: String::new(),
        status: String::new(),
        unread: 0,
        pinned: false,
        can_pin_messages: true,
        kind: ChatKind::Private,
        folders: vec![0],
    }];
    current.active_chat = Some(0);
    current.messages = vec![message(1, "first message"), message(2, "second message")];
    current
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
