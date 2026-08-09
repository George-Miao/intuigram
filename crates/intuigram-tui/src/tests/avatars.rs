use super::*;
use crate::source::graphics::GraphicsProtocol;

#[test]
fn text_avatar_fallbacks_render_in_chat_list_header_and_transcript() {
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

    let frame = render_test_frame(&current, 100, 40);
    let rendered = frame
        .buffer
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    assert_eq!(rendered.matches("[IT]").count(), 2);
    assert!(rendered.contains("[LQ]"));
    assert!(rendered.contains("12:34"));
    assert!(rendered.contains("[LQ] daily driver"));
}

#[test]
fn decoded_avatar_images_replace_badges_in_every_visible_peer_position() {
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
        .map(|peer| intuigram_app::AvatarView {
            avatar: intuigram_app::AvatarRef {
                peer,
                id: intuigram_app::AvatarId(peer.0),
            },
            image: intuigram_app::InlineImage::from_rgba(1, 1, vec![255, 0, 0, 255])
                .expect("fixture dimensions should match"),
        })
        .collect();

    let (frame, _) = render_test_frame_with_graphics(&current, 100, 40, GraphicsProtocol::Text);
    let rendered = frame
        .buffer
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    assert!(!rendered.contains("[IT]"));
    assert!(!rendered.contains("[LQ]"));
    assert!(rendered.matches('▀').count() >= 8);
}
