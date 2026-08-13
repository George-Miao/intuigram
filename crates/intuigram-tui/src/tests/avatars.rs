use super::*;
use crate::source::graphics::GraphicsProtocol;

#[test]
fn avatar_tiles_distinguish_loading_from_fallback() {
    let mut current = view(Vec::new());
    current.chats = vec![ChatView {
        id: ChatId(10),
        title: "Intuigram Team".to_owned(),
        preview: "daily driver".to_owned(),
        preview_sender: Some("Lin Qiao".to_owned()),
        preview_sender_peer: None,
        preview_timestamp: "12:34".to_owned(),
        status: "3 members".to_owned(),
        unread: 0,
        pinned: false,
        can_pin_messages: true,
        has_topics: false,
        has_direct_messages: false,
        kind: ChatKind::Supergroup,
        folders: vec![0],
    }];
    current.active_chat = Some(0);
    current.messages = vec![MessageView {
        id: MessageId(1),
        sender: "Lin Qiao".to_owned(),
        body: "hello".to_owned(),
        timestamp: "12:00".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: None,
        details: MessageDetails::default(),
    }];
    current.avatar_loads = vec![intuigram_lib::AvatarRef {
        peer: ChatId(10),
        id: intuigram_lib::AvatarId(1),
    }];

    let frame = render_test_frame(&current, 100, 40);
    let loading = Color::Rgb(128, 128, 128);
    let fallback = Color::Rgb(159, 116, 196);
    let loading_cells = frame
        .buffer
        .content
        .iter()
        .filter(|cell| cell.symbol() == " " && cell.bg == loading)
        .count();
    let fallback_cells = frame
        .buffer
        .content
        .iter()
        .filter(|cell| cell.symbol() == " " && cell.bg == fallback)
        .count();

    assert_eq!(loading_cells, 16);
    assert_eq!(fallback_cells, 8);
    assert!(
        frame.buffer.content.iter().all(|cell| cell.symbol() != "█"),
        "avatar tiles must fill cell backgrounds instead of relying on font block coverage"
    );
    let rendered = frame
        .buffer
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(!rendered.contains("[IT]"));
    assert!(!rendered.contains("[LQ]"));
    assert!(rendered.contains("12:34"));
    assert!(rendered.contains("Lin Qiao: daily driver"));
}

#[test]
fn message_avatar_spans_two_aligned_lead_rows() {
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
        has_direct_messages: false,
        kind: ChatKind::Supergroup,
        folders: vec![0],
    }];
    current.active_chat = Some(0);
    current.messages = vec![MessageView {
        id: MessageId(2),
        sender: "Berrysoft".to_owned(),
        body: "response".to_owned(),
        timestamp: "12:00".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: None,
        details: MessageDetails {
            sender_peer: Some(ChatId(20)),
            ..MessageDetails::default()
        },
    }];
    current.avatar_loads = vec![intuigram_lib::AvatarRef {
        peer: ChatId(20),
        id: intuigram_lib::AvatarId(1),
    }];

    let frame = render_test_frame(&current, 100, 40);
    let loading = Color::Rgb(128, 128, 128);
    let rows = (0..frame.buffer.area.height)
        .filter(|row| {
            (0..frame.buffer.area.width).any(|column| frame.buffer[(column, *row)].bg == loading)
        })
        .collect::<Vec<_>>();

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1], rows[0] + 1);
    let rendered = (0..frame.buffer.area.height)
        .map(|row| {
            (0..frame.buffer.area.width)
                .map(|column| frame.buffer[(column, row)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    let heading_row = rendered
        .iter()
        .position(|row| row.contains("Berrysoft"))
        .expect("sender heading should render");
    let body_row = rendered
        .iter()
        .position(|row| row.contains("response"))
        .expect("Message body should render");
    assert_eq!(usize::from(rows[0]), heading_row);
    assert_eq!(usize::from(rows[1]), body_row);
    let heading = &rendered[heading_row];
    let body = &rendered[body_row];
    assert_eq!(
        heading.find("Berrysoft"),
        body.find("response"),
        "the Message body should align with the sender heading"
    );
}

#[test]
fn decoded_avatar_images_replace_tiles_in_every_visible_peer_position() {
    let mut current = view(Vec::new());
    current.chats = vec![ChatView {
        id: ChatId(10),
        title: "Intuigram Team".to_owned(),
        preview: "daily driver".to_owned(),
        preview_sender: Some("Lin Qiao".to_owned()),
        preview_sender_peer: Some(ChatId(20)),
        preview_timestamp: "12:34".to_owned(),
        status: "3 members".to_owned(),
        unread: 0,
        pinned: false,
        can_pin_messages: true,
        has_topics: false,
        has_direct_messages: false,
        kind: ChatKind::Supergroup,
        folders: vec![0],
    }];
    current.active_chat = Some(0);
    current.messages = vec![MessageView {
        id: MessageId(1),
        sender: "Lin Qiao".to_owned(),
        body: "hello".to_owned(),
        timestamp: "12:00".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: None,
        details: MessageDetails {
            sender_peer: Some(ChatId(20)),
            ..MessageDetails::default()
        },
    }];
    current.avatars = [ChatId(10), ChatId(20)]
        .into_iter()
        .map(|peer| intuigram_lib::AvatarView {
            avatar: intuigram_lib::AvatarRef {
                peer,
                id: intuigram_lib::AvatarId(peer.0),
            },
            image: intuigram_lib::InlineImage::from_rgba(1, 1, vec![255, 0, 0, 255])
                .expect("fixture dimensions should match"),
        })
        .collect();

    let (frame, graphics) =
        render_test_frame_with_graphics(&current, 100, 40, GraphicsProtocol::KittyUnicode);
    let rendered = frame
        .buffer
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    assert!(!rendered.contains("[IT]"));
    assert!(!rendered.contains("[LQ]"));
    assert_eq!(graphics.requests().len(), 3);
    assert_eq!(
        graphics
            .requests()
            .iter()
            .filter(|request| request.size.columns == 4 && request.size.rows == 2)
            .count(),
        3,
        "two-row avatars should occupy a square pixel area"
    );
}

#[test]
fn channel_preview_omits_its_single_author_name() {
    let mut current = view(Vec::new());
    current.chats = vec![ChatView {
        id: ChatId(10),
        title: "Release channel".to_owned(),
        preview: "version 1.0".to_owned(),
        preview_sender: Some("Release bot".to_owned()),
        preview_sender_peer: Some(ChatId(20)),
        preview_timestamp: "12:34".to_owned(),
        status: "channel".to_owned(),
        unread: 0,
        pinned: false,
        can_pin_messages: true,
        has_topics: false,
        has_direct_messages: false,
        kind: ChatKind::Channel,
        folders: vec![0],
    }];
    current.active_chat = Some(0);

    let frame = render_test_frame(&current, 100, 40);
    let rendered = frame
        .buffer
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    assert!(rendered.contains("version 1.0"));
    assert!(!rendered.contains("Release bot: version 1.0"));
}
