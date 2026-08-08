#[test]
fn transcript_keeps_media_card_fallback_visible_beside_a_caption() {
    let mut view = view(Vec::new());
    view.chats = vec![ChatView {
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
    view.active_chat = Some(0);
    view.messages = vec![MessageView {
        id: MessageId(1),
        sender: "Lin".to_owned(),
        body: "caption".to_owned(),
        timestamp: "12:00".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: None,
        details: MessageDetails {
            media: Some(MediaCard {
                kind: MediaKind::Unsupported,
                title: "Unsupported Content".to_owned(),
                description: "constructor retained".to_owned(),
                details: Vec::new(),
                poll: None,
                remote_id: None,
            }),
            ..MessageDetails::default()
        },
    }];

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

    assert!(rendered.contains("Unsupported Content"));
    assert!(rendered.contains("constructor retained"));
}

#[test]
fn everforest_light_palette_is_used_for_the_terminal_surface() {
    let backend = TestBackend::new(100, 24);
    let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
    terminal
        .draw(|frame| render(frame, &view(Vec::new()), &EffectiveKeymap::defaults()))
        .expect("view should render");
    let buffer = terminal.backend().buffer();

    assert_eq!(buffer[(30, 5)].bg, Color::Rgb(253, 246, 227));
    assert_eq!(buffer[(5, 5)].bg, Color::Rgb(230, 226, 204));
    assert_eq!(buffer[(5, 5)].fg, Color::Rgb(92, 106, 114));
}

#[test]
fn active_folder_is_bold_and_underlined_without_a_selection_rule() {
    let mut view = view(Vec::new());
    view.folders = vec![
        FolderView {
            id: 0,
            title: "All".to_owned(),
            unread: 0,
        },
        FolderView {
            id: -1,
            title: "Archive".to_owned(),
            unread: 0,
        },
    ];
    let backend = TestBackend::new(100, 28);
    let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
    terminal
        .draw(|frame| render(frame, &view, &EffectiveKeymap::defaults()))
        .expect("view should render");
    let buffer = terminal.backend().buffer();
    let folder_row = 23;
    let all_x = (0..20)
        .find(|x| buffer[(*x, folder_row)].symbol() == "A")
        .expect("All Folder should render");
    let archive_x = (all_x + 1..30)
        .find(|x| buffer[(*x, folder_row)].symbol() == "A")
        .expect("Archive Folder should render");

    assert!((0..archive_x).all(|x| buffer[(x, folder_row)].symbol() != "│"));
    assert!(
        buffer[(all_x, folder_row)]
            .modifier
            .contains(Modifier::BOLD)
    );
    assert!(
        buffer[(all_x, folder_row)]
            .modifier
            .contains(Modifier::UNDERLINED)
    );
    assert!(
        !buffer[(archive_x, folder_row)]
            .modifier
            .contains(Modifier::UNDERLINED)
    );
}

#[test]
fn redrawing_shorter_chat_text_clears_the_previous_frame() {
    let mut view = view(Vec::new());
    view.chats = vec![ChatView {
        id: ChatId(10),
        title: "A title with stale trailing characters".to_owned(),
        preview: "A preview with stale trailing characters".to_owned(),
        status: String::new(),
        unread: 0,
        pinned: false,
        can_pin_messages: true,
        kind: ChatKind::Private,
        folders: vec![0],
    }];
    view.active_chat = Some(0);
    let backend = TestBackend::new(100, 24);
    let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
    terminal
        .draw(|frame| render(frame, &view, &EffectiveKeymap::defaults()))
        .expect("long view should render");

    view.chats[0].title = "X".to_owned();
    view.chats[0].preview.clear();
    terminal
        .draw(|frame| render(frame, &view, &EffectiveKeymap::defaults()))
        .expect("short view should render");
    let buffer = terminal.backend().buffer();

    let title_row = (0..30).map(|x| buffer[(x, 4)].symbol()).collect::<String>();
    assert!(
        (8..30).all(|x| buffer[(x, 4)].symbol() == " "),
        "stale title row: {title_row:?}"
    );
    assert!((3..30).all(|x| buffer[(x, 5)].symbol() == " "));
}

#[test]
fn side_by_side_render_separates_sections_and_highlights_the_interaction_target() {
    let mut view = view(vec![
        Action::Send,
        Action::Cancel,
        Action::TargetPreviousMessage,
    ]);
    view.account_name = "Ada".to_owned();
    view.folders = vec![FolderView {
        id: 0,
        title: "All".to_owned(),
        unread: 1,
    }];
    view.chats = vec![ChatView {
        id: ChatId(10),
        title: "Intuigram".to_owned(),
        preview: "daily driver".to_owned(),
        status: String::new(),
        unread: 1,
        pinned: true,
        can_pin_messages: true,
        kind: ChatKind::Supergroup,
        folders: vec![0],
    }];
    view.active_chat = Some(0);
    view.messages = vec![MessageView {
        id: MessageId(1),
        sender: "Lin".to_owned(),
        body: "hello".to_owned(),
        timestamp: "12:00".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: Some(MessageId(0)),
        details: MessageDetails::default(),
    }];
    view.active_message = Some(0);
    view.focus = Focus::Composer;

    let backend = TestBackend::new(100, 28);
    let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
    terminal
        .draw(|frame| render(frame, &view, &EffectiveKeymap::defaults()))
        .expect("view should render");
    let rendered = render_test_frame(&view, 100, 28);
    let buffer = terminal.backend().buffer();
    let message = rendered
        .semantics
        .iter()
        .find(|node| node.role == SemanticRole::Message)
        .expect("the active Message should have matching semantics");

    assert_eq!(buffer[(0, 4)].symbol(), "│");
    assert_eq!(buffer[(32, message.bounds.top())].symbol(), "│");
    assert_eq!(buffer[(34, message.bounds.top() + 1)].symbol(), "│");
    assert_eq!(
        buffer[(34, message.bounds.top() + 1)].fg,
        Color::Rgb(58, 148, 197)
    );
    assert_eq!(buffer[(32, 20)].symbol(), "│");
    assert_eq!(buffer[(34, 20)].symbol(), "T");
    assert_eq!(buffer[(5, 5)].bg, Color::Rgb(253, 246, 227));
    assert_eq!(buffer[(40, 5)].bg, Color::Rgb(253, 246, 227));
    assert_eq!(buffer[(40, 19)].bg, Color::Rgb(230, 226, 204));
    assert_eq!(buffer[(40, 20)].bg, Color::Rgb(230, 226, 204));
    assert_eq!(buffer[(40, 21)].bg, Color::Rgb(230, 226, 204));
    assert_eq!(buffer[(31, 19)].bg, Color::Rgb(230, 226, 204));
    assert_eq!(buffer[(99, 21)].bg, Color::Rgb(230, 226, 204));
    assert_eq!(buffer[(30, 5)].bg, Color::Rgb(253, 246, 227));
    assert_eq!(buffer[(5, 21)].bg, Color::Rgb(253, 246, 227));
    assert_eq!(buffer[(5, 22)].bg, Color::Rgb(253, 246, 227));
    assert_eq!(buffer[(5, 23)].bg, Color::Rgb(253, 246, 227));
    assert_eq!(buffer[(5, 26)].bg, Color::Rgb(253, 246, 227));
    assert!(
        buffer
            .content
            .iter()
            .all(|cell| { !matches!(cell.symbol(), "┌" | "┐" | "└" | "┘" | "─") })
    );

    view.focus = Focus::Chats;
    terminal
        .draw(|frame| render(frame, &view, &EffectiveKeymap::defaults()))
        .expect("Chat-list focus should render");
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(5, 5)].bg, Color::Rgb(230, 226, 204));
    assert_eq!(buffer[(40, 19)].bg, Color::Rgb(253, 246, 227));
    assert_eq!(buffer[(40, 21)].bg, Color::Rgb(253, 246, 227));

    view.focus = Focus::Transcript;
    terminal
        .draw(|frame| render(frame, &view, &EffectiveKeymap::defaults()))
        .expect("Transcript focus should render");
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(40, 5)].bg, Color::Rgb(230, 226, 204));
    assert_eq!(buffer[(40, 19)].bg, Color::Rgb(253, 246, 227));
    assert_eq!(buffer[(40, 21)].bg, Color::Rgb(253, 246, 227));

    view.focus = Focus::Search;
    terminal
        .draw(|frame| render(frame, &view, &EffectiveKeymap::defaults()))
        .expect("search focus should render");
    assert_eq!(
        terminal.backend().buffer()[(5, 26)].bg,
        Color::Rgb(230, 226, 204)
    );
}

#[test]
fn wide_layout_does_not_render_empty_details() {
    let backend = TestBackend::new(140, 30);
    let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
    terminal
        .draw(|frame| render(frame, &view(Vec::new()), &EffectiveKeymap::defaults()))
        .expect("view should render");

    let rendered: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(!rendered.contains("Details"));
}

#[test]
fn transcript_renders_rich_metadata_album_and_quiz_fallbacks() {
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
    current.messages = vec![
        rich_album_message(1, "first"),
        rich_album_message(2, "second"),
    ];
    current.messages.push(MessageView {
        id: MessageId(3),
        sender: "Lin".to_owned(),
        body: "Choose".to_owned(),
        timestamp: "12:02".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: Some(MessageId(1)),
        details: MessageDetails {
            forwarded_from: Some("Runtime News".to_owned()),
            reactions: vec![ReactionView {
                label: "👍".to_owned(),
                count: 4,
                chosen: true,
            }],
            edited: true,
            pinned: true,
            views: Some(10),
            replies: Some(2),
            media: Some(MediaCard {
                kind: MediaKind::Poll,
                title: "Quiz".to_owned(),
                description: "Which runtime?".to_owned(),
                details: Vec::new(),
                poll: Some(PollView {
                    quiz: true,
                    multiple_choice: false,
                    closed: true,
                    total_voters: Some(5),
                    options: vec![PollOptionView {
                        text: "Compio".to_owned(),
                        voters: Some(3),
                        chosen: true,
                        correct: true,
                    }],
                    solution: Some("Completion-based I/O".to_owned()),
                }),
                remote_id: Some("77".to_owned()),
            }),
            ..MessageDetails::default()
        },
    });

    let backend = TestBackend::new(120, 36);
    let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
    terminal
        .draw(|frame| render(frame, &current, &EffectiveKeymap::defaults()))
        .expect("rich transcript should render");
    let rendered = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    for expected in [
        "Album 1 · Photo",
        "Album end · Photo",
        "Forwarded from Runtime News",
        "edited",
        "pinned",
        "10 views",
        "2 replies",
        "Compio · 3",
        "5 voters · closed",
        "Explanation: Completion-based I/O",
    ] {
        assert!(rendered.contains(expected), "missing {expected:?}");
    }
}

fn rich_album_message(id: i64, body: &str) -> MessageView {
    MessageView {
        id: MessageId(id),
        sender: "Lin".to_owned(),
        body: body.to_owned(),
        timestamp: "12:00".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: None,
        details: MessageDetails {
            entities: vec![TextEntity {
                offset: 0,
                length: body.len(),
                kind: TextEntityKind::Bold,
            }],
            media: Some(MediaCard {
                kind: MediaKind::Photo,
                title: "Photo".to_owned(),
                description: "image".to_owned(),
                details: Vec::new(),
                poll: None,
                remote_id: Some(id.to_string()),
            }),
            album_id: Some(99),
            ..MessageDetails::default()
        },
    }
}
use super::*;
