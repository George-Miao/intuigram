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
        has_topics: false,
        has_direct_messages: false,
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

    assert_eq!(
        rendered_rows(&frame.buffer)
            .iter()
            .filter(|row| row.trim_end().ends_with("Lin"))
            .count(),
        1
    );
    assert_eq!(
        rendered_rows(&frame.buffer)
            .iter()
            .filter(|row| row.trim_end().ends_with("Ada"))
            .count(),
        1
    );
    assert!(rendered.contains("2026-08-07"));
    assert!(rendered.contains("2026-08-08"));
    assert!(rendered.contains("│ Lin: first"));
}

#[test]
fn reply_body_keeps_avatar_indent_after_reference() {
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
    let mut reply = message(2, "Eric", "response", "2026-08-07");
    reply.reply_to = Some(MessageId(1));
    reply.details.sender_peer = Some(ChatId(20));
    current.messages = vec![message(1, "Lin", "original", "2026-08-07"), reply];

    let rows = rendered_rows(&render_test_frame(&current, 100, 40).buffer);
    let sender = rows
        .iter()
        .find(|row| row.trim_end().ends_with("Eric"))
        .expect("reply sender heading should render");
    let reference = rows
        .iter()
        .find(|row| row.contains("│ Lin: original"))
        .expect("reply reference should render");
    let body = rows
        .iter()
        .find(|row| row.contains("response"))
        .expect("reply body should render");
    let sender_column = sender
        .find("Eric")
        .map(|index| sender[..index].chars().count());
    let body_column = body
        .find("response")
        .map(|index| body[..index].chars().count());
    let reference_column = reference
        .find("Lin: original")
        .map(|index| reference[..index].chars().count());
    assert_eq!(
        body_column, sender_column,
        "reply body should retain its message-column indent"
    );
    assert_eq!(
        reference_column,
        body_column.map(|column| column + 2),
        "reply text should sit inside its own rule"
    );
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

fn rendered_rows(buffer: &ratatui::buffer::Buffer) -> Vec<String> {
    (0..buffer.area.height)
        .map(|row| {
            (0..buffer.area.width)
                .map(|column| buffer[(column, row)].symbol())
                .collect()
        })
        .collect()
}
