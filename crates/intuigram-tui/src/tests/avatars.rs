use super::*;

#[test]
fn text_avatar_fallbacks_render_in_chat_list_header_and_transcript() {
    let mut current = view(Vec::new());
    current.chats = vec![ChatView {
        id: ChatId(10),
        title: "Intuigram Team".to_owned(),
        preview: "daily driver".to_owned(),
        status: "3 members".to_owned(),
        unread: 0,
        pinned: false,
        can_pin_messages: true,
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
}
