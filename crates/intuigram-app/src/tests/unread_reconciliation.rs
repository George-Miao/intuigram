use super::*;

#[test]
fn unread_boundary_is_stable_across_history_and_live_updates_until_read() {
    let mut app = App::new();
    let fixture = bootstrap();
    let recent = fixture.messages.clone();
    apply(&mut app, Input::Adapter(AdapterEvent::Bootstrap(fixture)));

    assert_eq!(app.view().unread_boundary, Some(MessageId(2)));

    let mut refreshed = vec![MessageView {
        id: MessageId(0),
        sender: "Lin".to_owned(),
        body: "older".to_owned(),
        timestamp: "11:59".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: None,
        details: MessageDetails::default(),
    }];
    refreshed.extend(recent);
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::ChatLoaded {
            chat: ChatId(10),
            status: None,
            messages: refreshed,
            pinned_messages: Vec::new(),
        }),
    );
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::MessageAdded {
            chat: ChatId(10),
            message: Box::new(MessageView {
                id: MessageId(4),
                sender: "Lin".to_owned(),
                body: "live".to_owned(),
                timestamp: "12:01".to_owned(),
                direction: MessageDirection::Incoming,
                delivery: DeliveryState::Read,
                reply_to: None,
                details: MessageDetails::default(),
            }),
        }),
    );

    assert_eq!(app.view().unread_boundary, Some(MessageId(2)));

    apply(
        &mut app,
        Input::Adapter(AdapterEvent::HistoryRead {
            chat: ChatId(10),
            saved_peer: None,
            max_id: MessageId(3),
            outgoing: false,
            unread: Some(1),
        }),
    );
    assert_eq!(app.view().unread_boundary, Some(MessageId(4)));

    apply(
        &mut app,
        Input::Adapter(AdapterEvent::HistoryRead {
            chat: ChatId(10),
            saved_peer: None,
            max_id: MessageId(4),
            outgoing: false,
            unread: Some(0),
        }),
    );
    assert_eq!(app.view().unread_boundary, None);
}
