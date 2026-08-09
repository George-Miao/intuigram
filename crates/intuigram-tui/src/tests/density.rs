use super::*;
use crate::source::render_layout::capped_text;

#[test]
fn chat_text_cap_counts_terminal_cells_and_keeps_three_dots_inside_the_width() {
    let capped = capped_text("终端界面终端界面", 12);

    assert_eq!(ratatui::text::Line::from(capped.as_str()).width(), 11);
    assert!(capped.ends_with("..."));
}

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
            preview_sender: None,
            preview_sender_peer: None,
            preview_timestamp: String::new(),
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
            preview_sender: None,
            preview_sender_peer: None,
            preview_timestamp: String::new(),
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

    let rendered = render_test_frame_with_mode(&current, 100, 24, ViewMode::Compact);
    let buffer = &rendered.buffer;
    let messages = rendered
        .semantics
        .iter()
        .filter(|node| node.role == SemanticRole::Message)
        .collect::<Vec<_>>();

    assert_eq!(buffer[(7, 2)].symbol(), "F");
    assert_eq!(buffer[(7, 4)].symbol(), "S");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].bounds.height, 2);
    assert_eq!(messages[0].bounds.bottom(), messages[1].bounds.top());
    assert_eq!(messages[1].bounds.height, 2);
    assert_eq!(buffer[(2, 22)].symbol(), "A");
}

#[test]
fn default_view_pads_folder_and_combined_chrome_regions_on_all_sides() {
    let current = view(vec![Action::Quit]);
    let rendered = render_test_frame(&current, 100, 28);
    let buffer = &rendered.buffer;

    assert_eq!(buffer[(1, 22)].symbol(), " ");
    assert_eq!(buffer[(1, 23)].symbol(), " ");
    assert_eq!(buffer[(1, 24)].symbol(), " ");
    assert_eq!(buffer[(1, 25)].symbol(), " ");
    assert_eq!(buffer[(1, 26)].symbol(), "c");
    assert_eq!(buffer[(1, 27)].symbol(), " ");
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

    assert_eq!(normal_chat.bounds.width, 30);
    assert_eq!(normal_message.bounds.x, 34);
    assert_eq!(wide_chat.bounds.width, 38);
    assert_eq!(wide_message.bounds.x, 42);
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
