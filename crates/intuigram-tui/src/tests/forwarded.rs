use super::*;

#[test]
fn forwarded_provenance_rule_spans_source_caption_and_media() {
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
        kind: ChatKind::Private,
        folders: vec![0],
    }];
    current.active_chat = Some(0);
    current.messages = vec![MessageView {
        id: MessageId(40),
        sender: "Lin".to_owned(),
        body: "a forwarded caption".to_owned(),
        timestamp: "12:00".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: None,
        details: MessageDetails {
            sender_peer: None,
            forwarded_from: Some("Runtime News".to_owned()),
            media: Some(MediaCard {
                kind: MediaKind::Photo,
                title: "Photo".to_owned(),
                description: "image/png".to_owned(),
                details: vec!["1280×720".to_owned()],
                poll: None,
                specialized: None,
                remote_id: Some("photo:40".to_owned()),
            }),
            ..MessageDetails::default()
        },
    }];

    let frame = render_test_frame(&current, 100, 40);
    let rows = rendered_rows(&frame.buffer);
    for content in [
        "Forwarded from Runtime News",
        "a forwarded caption",
        "[Photo]",
        "1280×720",
    ] {
        let row = rows
            .iter()
            .find(|row| row.contains(content))
            .unwrap_or_else(|| panic!("missing forwarded content {content:?}"));
        assert!(row.contains("│ "), "missing provenance bar in {row:?}");
    }

    let sender = rows
        .iter()
        .position(|row| row.trim_end().ends_with("Lin"))
        .expect("the sender heading should render");
    let forwarded = rows
        .iter()
        .position(|row| row.contains("Forwarded from Runtime News"))
        .expect("the forwarded provenance should render");
    assert_eq!(forwarded, sender + 1);
    assert!(
        rows[forwarded]
            .find("Forwarded from Runtime News")
            .is_some_and(|column| column > 0),
        "the provenance should share the avatar's second row"
    );
    let caption = rows
        .iter()
        .position(|row| row.contains("a forwarded caption"))
        .expect("the forwarded caption should render");
    assert_eq!(
        caption,
        forwarded + 2,
        "forwarded content should have one line of top padding"
    );
    assert_eq!(
        rows[forwarded + 1].trim(),
        "│",
        "the provenance rule should span the top padding"
    );
}

#[test]
fn forwarded_rule_aligns_with_a_reply_rule_beside_message_content() {
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
        kind: ChatKind::Private,
        folders: vec![0],
    }];
    current.active_chat = Some(0);
    current.messages = vec![
        MessageView {
            id: MessageId(1),
            sender: "Lin".to_owned(),
            body: "original".to_owned(),
            timestamp: "11:58".to_owned(),
            direction: MessageDirection::Incoming,
            delivery: DeliveryState::Read,
            reply_to: None,
            details: MessageDetails::default(),
        },
        MessageView {
            id: MessageId(2),
            sender: "Lin".to_owned(),
            body: "forwarded".to_owned(),
            timestamp: "11:59".to_owned(),
            direction: MessageDirection::Incoming,
            delivery: DeliveryState::Read,
            reply_to: None,
            details: MessageDetails {
                forwarded_from: Some("Runtime News".to_owned()),
                ..MessageDetails::default()
            },
        },
        MessageView {
            id: MessageId(3),
            sender: "Lin".to_owned(),
            body: "reply".to_owned(),
            timestamp: "12:00".to_owned(),
            direction: MessageDirection::Incoming,
            delivery: DeliveryState::Read,
            reply_to: Some(MessageId(1)),
            details: MessageDetails::default(),
        },
    ];

    let rows = rendered_rows(&render_test_frame(&current, 100, 40).buffer);
    let forwarded = rows
        .iter()
        .find(|row| row.contains("Forwarded from Runtime News"))
        .expect("forwarded provenance should render");
    let reply = rows
        .iter()
        .find(|row| row.contains("Lin: original"))
        .expect("reply preview should render");
    let forwarded_rule = forwarded
        .chars()
        .position(|symbol| symbol == '│')
        .expect("forwarded provenance should have a rule");
    let reply_rule = reply
        .chars()
        .position(|symbol| symbol == '│')
        .expect("reply preview should have a rule");

    assert_eq!(forwarded_rule, reply_rule);
    assert_eq!(
        forwarded
            .match_indices("Forwarded from Runtime News")
            .next()
            .map(|(index, _)| forwarded[..index].chars().count()),
        Some(forwarded_rule + 2)
    );
    assert_eq!(
        reply
            .match_indices("Lin: original")
            .next()
            .map(|(index, _)| reply[..index].chars().count()),
        Some(reply_rule + 2)
    );
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
