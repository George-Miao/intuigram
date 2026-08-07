use super::*;

#[test]
fn forwarded_provenance_rule_spans_source_caption_and_media() {
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
    current.messages = vec![MessageView {
        id: MessageId(40),
        sender: "Lin".to_owned(),
        body: "a forwarded caption".to_owned(),
        timestamp: "12:00".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: None,
        details: MessageDetails {
            forwarded_from: Some("Runtime News".to_owned()),
            media: Some(MediaCard {
                kind: MediaKind::Photo,
                title: "Photo".to_owned(),
                description: "image/png".to_owned(),
                details: vec!["1280×720".to_owned()],
                poll: None,
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
