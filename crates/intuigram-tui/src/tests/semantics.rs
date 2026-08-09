#[test]
fn semantic_nodes_are_generated_with_the_cells_they_describe() {
    let mut current = view(vec![Action::Open, Action::Quit]);
    current.chats = vec![ChatView {
        id: ChatId(10),
        title: "Rust".to_owned(),
        preview: "hello".to_owned(),
        preview_sender: None,
        preview_timestamp: String::new(),
        status: String::new(),
        unread: 0,
        pinned: false,
        can_pin_messages: true,
        kind: ChatKind::Private,
        folders: vec![0],
    }];
    current.active_chat = Some(0);
    let rendered = render_test_frame(&current, 100, 24);
    let chat = rendered
        .semantics
        .iter()
        .find(|node| node.role == SemanticRole::Chat)
        .expect("a visible Chat should have a semantic node");
    let mut cells = String::new();
    for y in chat.bounds.top()..chat.bounds.bottom() {
        for x in chat.bounds.left()..chat.bounds.right() {
            cells.push_str(rendered.buffer[(x, y)].symbol());
        }
    }

    assert_eq!(chat.domain_id, Some(10));
    assert!(chat.active);
    assert!(cells.contains("Rust"));
    assert!(
        rendered
            .semantics
            .iter()
            .any(|node| node.role == SemanticRole::Composer)
    );
    assert!(
        rendered
            .semantics
            .iter()
            .any(|node| { node.role == SemanticRole::Action && node.action == Some(Action::Quit) })
    );
}

#[test]
fn terminal_keyboard_protocol_disambiguates_modified_enter() {
    let flags = terminal_keyboard_flags();

    assert!(flags.contains(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES));
    assert!(flags.contains(KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES));
}

#[test]
fn folder_membership_overlay_shows_selection_and_current_membership() {
    let mut view = view(vec![
        Action::MoveUp,
        Action::MoveDown,
        Action::ToggleFolderMembership,
        Action::Cancel,
    ]);
    view.folders = vec![
        FolderView {
            id: 0,
            title: "All".to_owned(),
            unread: 0,
        },
        FolderView {
            id: 2,
            title: "Work".to_owned(),
            unread: 4,
        },
    ];
    view.chats = vec![ChatView {
        id: ChatId(10),
        title: "Intuigram".to_owned(),
        preview: String::new(),
        preview_sender: None,
        preview_timestamp: String::new(),
        status: String::new(),
        unread: 0,
        pinned: false,
        can_pin_messages: true,
        kind: ChatKind::Private,
        folders: vec![0, 2],
    }];
    view.active_chat = Some(0);
    view.folder_picker = Some(1);

    let backend = TestBackend::new(100, 24);
    let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
    terminal
        .draw(|frame| render(frame, &view, &EffectiveKeymap::defaults()))
        .expect("view should render");
    let rendered = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    assert!(rendered.contains("Folder membership"));
    assert!(rendered.contains("[x] Work"));
}

#[test]
fn chat_list_scroll_waits_for_each_directional_cap() {
    let mut view = view(Vec::new());
    view.chats = (0..14)
        .map(|index| ChatView {
            id: ChatId(index),
            title: format!("Chat {index}"),
            preview: format!("Preview {index}"),
            preview_sender: None,
            preview_timestamp: String::new(),
            status: String::new(),
            unread: 0,
            pinned: false,
            can_pin_messages: true,
            kind: ChatKind::Private,
            folders: vec![0],
        })
        .collect();
    view.active_chat = Some(8);
    view.chat_scroll_direction = ScrollDirection::Down;
    let mut renderer = TestRenderer::default();
    let down = renderer.render(&view, 100, 24);
    let down_chats = visible_chat_names(&down);

    view.active_chat = Some(7);
    view.chat_scroll_direction = ScrollDirection::Up;
    let before_cap = renderer.render(&view, 100, 24);
    assert_eq!(visible_chat_names(&before_cap), down_chats);

    view.active_chat = Some(6);
    let scrolled = renderer.render(&view, 100, 24);
    assert_ne!(visible_chat_names(&scrolled), down_chats);
}

fn visible_chat_names(frame: &TestFrame) -> Vec<&str> {
    frame
        .semantics
        .iter()
        .filter(|node| node.role == SemanticRole::Chat)
        .map(|node| node.name.as_str())
        .collect()
}

#[test]
fn transcript_scroll_keeps_the_active_message_visible() {
    let mut view = view(Vec::new());
    view.chats = vec![ChatView {
        id: ChatId(10),
        title: "Intuigram".to_owned(),
        preview: String::new(),
        preview_sender: None,
        preview_timestamp: String::new(),
        status: String::new(),
        unread: 0,
        pinned: false,
        can_pin_messages: true,
        kind: ChatKind::Private,
        folders: vec![0],
    }];
    view.active_chat = Some(0);
    view.messages = (0..20)
        .map(|index| MessageView {
            id: MessageId(index),
            sender: "Lin".to_owned(),
            body: format!("Message {index}"),
            timestamp: "12:00".to_owned(),
            direction: MessageDirection::Incoming,
            delivery: DeliveryState::Read,
            reply_to: None,
            details: MessageDetails::default(),
        })
        .collect();
    view.active_message = Some(10);
    view.focus = Focus::Transcript;

    let rendered = render_test_frame(&view, 100, 28);
    let active = rendered
        .semantics
        .iter()
        .find(|node| node.role == SemanticRole::Message && node.domain_id == Some(10))
        .expect("active Message should remain visible");

    assert_eq!(rendered.buffer[(34, active.bounds.y)].symbol(), "│");
    assert_eq!(rendered.buffer[(36, active.bounds.y)].symbol(), "M");
}

#[test]
fn transcript_scroll_preserves_an_inactive_anchor() {
    let mut view = view(Vec::new());
    view.chats = vec![ChatView {
        id: ChatId(10),
        title: "Intuigram".to_owned(),
        preview: String::new(),
        preview_sender: None,
        preview_timestamp: String::new(),
        status: String::new(),
        unread: 0,
        pinned: false,
        can_pin_messages: true,
        kind: ChatKind::Private,
        folders: vec![0],
    }];
    view.active_chat = Some(0);
    view.messages = (0..20)
        .map(|index| MessageView {
            id: MessageId(index),
            sender: "Lin".to_owned(),
            body: format!("Message {index}"),
            timestamp: "12:00".to_owned(),
            direction: MessageDirection::Incoming,
            delivery: DeliveryState::Read,
            reply_to: None,
            details: MessageDetails::default(),
        })
        .collect();
    view.transcript_anchor = Some(10);
    view.focus = Focus::Composer;

    let backend = TestBackend::new(100, 24);
    let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
    terminal
        .draw(|frame| render(frame, &view, &EffectiveKeymap::defaults()))
        .expect("view should render");
    let buffer = terminal.backend().buffer();

    assert_eq!(buffer[(34, 11)].symbol(), " ");
    assert_eq!(buffer[(36, 12)].symbol(), "M");
}

#[test]
fn a_short_latest_transcript_is_anchored_above_the_composer() {
    let mut current = view(Vec::new());
    current.chats = vec![ChatView {
        id: ChatId(10),
        title: "Intuigram".to_owned(),
        preview: String::new(),
        preview_sender: None,
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
    current.messages = vec![MessageView {
        id: MessageId(1),
        sender: "Lin".to_owned(),
        body: "latest".to_owned(),
        timestamp: "12:00".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: None,
        details: MessageDetails::default(),
    }];

    let rendered = render_test_frame(&current, 100, 40);
    let message = rendered
        .semantics
        .iter()
        .find(|node| node.role == SemanticRole::Message)
        .expect("the latest Message should be visible");
    let composer = rendered
        .semantics
        .iter()
        .find(|node| node.role == SemanticRole::Composer)
        .expect("the Composer should be visible");

    assert!(
        composer
            .bounds
            .top()
            .saturating_sub(message.bounds.bottom())
            <= 1
    );
}
use super::*;
