use super::*;
use crate::source::graphics::GraphicsProtocol;

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
        preview_sender: Some("Lin Qiao".to_owned()),
        preview_sender_peer: None,
        preview_timestamp: "12:34".to_owned(),
        status: String::new(),
        unread: 1,
        pinned: true,
        can_pin_messages: true,
        has_topics: false,
        has_direct_messages: false,
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

    assert_eq!(buffer[(0, 4)].symbol(), "▌");
    assert_eq!(buffer[(34, message.bounds.top())].symbol(), "▌");
    let reply_row = message.bounds.top() + 1;
    let reply_rule = (message.bounds.left()..message.bounds.right())
        .find(|column| {
            buffer[(*column, reply_row)].symbol() == "│"
                && buffer[(*column, reply_row)].fg == Color::Rgb(58, 148, 197)
        })
        .expect("the reply rule should render beside the avatar's second row");
    assert!(reply_rule > message.bounds.left());
    assert_eq!(buffer[(34, 20)].symbol(), "│");
    assert_eq!(
        buffer[(34, message.bounds.bottom().saturating_sub(1))].symbol(),
        " "
    );
    assert_eq!(buffer[(36, 20)].symbol(), "T");
    assert_eq!(buffer[(10, 5)].bg, Color::Reset);
    assert_eq!(buffer[(40, 5)].bg, Color::Reset);
    assert_eq!(buffer[(40, 19)].bg, Color::Rgb(230, 226, 204));
    assert_eq!(buffer[(40, 20)].bg, Color::Rgb(230, 226, 204));
    assert_eq!(buffer[(40, 21)].bg, Color::Rgb(230, 226, 204));
    assert_eq!(buffer[(33, 19)].bg, Color::Rgb(230, 226, 204));
    assert_eq!(buffer[(99, 21)].bg, Color::Rgb(230, 226, 204));
    assert_eq!(buffer[(32, 5)].bg, Color::Reset);
    assert_eq!(buffer[(5, 21)].bg, Color::Reset);
    assert_eq!(buffer[(5, 22)].bg, Color::Reset);
    assert_eq!(buffer[(5, 23)].bg, Color::Reset);
    assert_eq!(buffer[(5, 26)].bg, Color::Reset);
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
    assert_eq!(buffer[(10, 5)].bg, Color::Rgb(230, 226, 204));
    assert_eq!(buffer[(40, 19)].bg, Color::Reset);
    assert_eq!(buffer[(40, 21)].bg, Color::Reset);

    view.focus = Focus::Transcript;
    terminal
        .draw(|frame| render(frame, &view, &EffectiveKeymap::defaults()))
        .expect("Transcript focus should render");
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(40, 5)].bg, Color::Rgb(230, 226, 204));
    assert_eq!(buffer[(40, 19)].bg, Color::Reset);
    assert_eq!(buffer[(40, 21)].bg, Color::Reset);

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
fn popup_suppresses_intersecting_graphics_placement() {
    let mut current = super::graphics::image_message_view();
    current.media_previews = vec![intuigram_lib::MediaPreviewView {
        chat: ChatId(10),
        message: MessageId(40),
        image: intuigram_lib::InlineImage::from_rgba(1, 1, vec![255, 0, 0, 255])
            .expect("fixture pixels should match their dimensions"),
    }];
    current.action_menu = Some(ActionMenuView {
        title: "Message Actions".to_owned(),
        selected: 0,
        items: vec![ActionMenuItemView {
            action: Action::Reply,
            label: "Reply".to_owned(),
        }],
    });

    let (_, graphics) =
        render_test_frame_with_graphics(&current, 54, 30, GraphicsProtocol::KittyUnicode);

    assert!(
        graphics.requests().is_empty(),
        "popup must cover the graphics placement"
    );
}
