#[test]
fn unread_divider_is_rendered_immediately_before_the_boundary_message() {
    let mut current = view(Vec::new());
    current.chats = vec![ChatView {
        id: ChatId(10),
        title: "Intuigram".to_owned(),
        preview: String::new(),
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

    let mut terminal =
        Terminal::new(TestBackend::new(100, 24)).expect("test terminal should initialize");
    terminal
        .draw(|frame| render(frame, &current, &EffectiveKeymap::defaults()))
        .expect("Transcript should render");
    let rows = (0..24)
        .map(|row| {
            (0..100)
                .map(|column| terminal.backend().buffer()[(column, row)].symbol())
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

    assert_eq!(divider + 2, unread);
}

use super::*;
