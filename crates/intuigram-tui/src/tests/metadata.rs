use super::*;

#[test]
fn message_metadata_is_right_aligned_and_omits_zero_counters_except_views() {
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
        body: "short message".to_owned(),
        timestamp: "12:34".to_owned(),
        direction: MessageDirection::Outgoing,
        delivery: DeliveryState::Sent,
        reply_to: None,
        details: MessageDetails {
            sender_peer: None,
            reactions: vec![ReactionView {
                label: "👍".to_owned(),
                count: 0,
                chosen: true,
            }],
            edited: true,
            views: Some(0),
            forwards: Some(0),
            replies: Some(0),
            ..MessageDetails::default()
        },
    }];

    let frame = render_test_frame(&current, 100, 40);
    let rows = rendered_rows(&frame.buffer);
    let metadata = rows
        .iter()
        .find(|row| row.contains("0 views"))
        .expect("view count should always render");
    assert!(metadata.contains("edited · 12:34 · ✓"));
    assert_eq!(
        metadata
            .chars()
            .rev()
            .take_while(|character| *character == ' ')
            .count(),
        1
    );
    assert!(rows.iter().all(|row| !row.contains("0 forwards")));
    assert!(rows.iter().all(|row| !row.contains("0 replies")));
    assert!(rows.iter().all(|row| !row.contains("👍 0")));
    let sender = rows
        .iter()
        .find(|row| row.contains("[LI] Lin"))
        .expect("sender header should render");
    assert!(!sender.contains("12:34"));
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
