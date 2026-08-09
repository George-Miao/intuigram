#[test]
fn unread_divider_is_rendered_immediately_before_the_boundary_message() {
    let mut current = view(Vec::new());
    current.chats = vec![ChatView {
        id: ChatId(10),
        title: "Intuigram".to_owned(),
        preview: String::new(),
        preview_sender: None,
        preview_timestamp: String::new(),
        status: String::new(),
        unread: 2,
        pinned: false,
        can_pin_messages: true,
        kind: ChatKind::Private,
        folders: vec![0],
    }];
    current.active_chat = Some(0);
    current.unread_boundary = Some(MessageId(41));
    current.messages = [(40, "read"), (41, "first unread"), (42, "second unread")]
        .into_iter()
        .map(|(id, body)| MessageView {
            id: MessageId(id),
            sender: "Lin".to_owned(),
            body: body.to_owned(),
            timestamp: "12:00".to_owned(),
            direction: MessageDirection::Incoming,
            delivery: DeliveryState::Read,
            reply_to: None,
            details: MessageDetails::default(),
        })
        .collect();

    let frame = render_test_frame(&current, 100, 24);
    let rows = (0..24)
        .map(|row| {
            (0..100)
                .map(|column| frame.buffer[(column, row)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    let divider = rows
        .iter()
        .position(|row| row.contains("Unread messages"))
        .expect("unread divider should render");
    let unread = rows
        .iter()
        .position(|row| row.contains("first unread"))
        .expect("boundary Message should render");

    assert_eq!(divider + 1, unread);
    let transcript = frame
        .semantics
        .iter()
        .find(|node| node.role == SemanticRole::Transcript)
        .expect("Transcript semantics should expose its padded bounds");
    let divider_column = rows[divider]
        .find("Unread messages")
        .expect("divider text should have a cell position");
    assert_eq!(
        divider_column,
        usize::from(transcript.bounds.x)
            + usize::from(transcript.bounds.width).saturating_sub("Unread messages".len()) / 2,
    );
}

use super::*;
