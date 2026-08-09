use super::*;

#[test]
fn transcript_groups_senders_and_shows_dates_and_reply_previews() {
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
        kind: ChatKind::Supergroup,
        folders: vec![0],
    }];
    current.active_chat = Some(0);
    current.messages = vec![
        message(1, "Lin", "first", "2026-08-07"),
        MessageView {
            reply_to: Some(MessageId(1)),
            ..message(2, "Lin", "second", "2026-08-07")
        },
        message(3, "Ada", "third", "2026-08-08"),
    ];

    let frame = render_test_frame(&current, 100, 40);
    let rendered = frame
        .buffer
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    assert_eq!(rendered.matches("[LI] Lin").count(), 1);
    assert_eq!(rendered.matches("[AD] Ada").count(), 1);
    assert!(rendered.contains("2026-08-07"));
    assert!(rendered.contains("2026-08-08"));
    assert!(rendered.contains("│ Lin: first"));
}

fn message(id: i64, sender: &str, body: &str, date: &str) -> MessageView {
    MessageView {
        id: MessageId(id),
        sender: sender.to_owned(),
        body: body.to_owned(),
        timestamp: "12:00".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: None,
        details: MessageDetails {
            date_label: date.to_owned(),
            ..MessageDetails::default()
        },
    }
}
