use super::*;

#[test]
fn edit_previous_is_a_noop_when_history_has_no_eligible_outgoing_message() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
    );
    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));

    let update = app.transition(Input::Intent(Intent::Action(Action::EditPrevious)));

    assert_eq!(update.view.focus, Focus::Composer);
    assert_eq!(update.view.composer, crate::ComposerView::default());
}

#[test]
fn animation_frames_advance_only_while_effort_is_pending() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
    );
    apply(&mut app, Input::Intent(Intent::Animate));
    assert_eq!(app.view().animation_frame, 0);

    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
    let messages = app.view().messages;
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::ChatLoaded {
            chat: ChatId(10),
            status: None,
            messages,
            pinned_messages: Vec::new(),
        }),
    );
    apply(
        &mut app,
        Input::Intent(Intent::Insert("sending".to_owned())),
    );
    apply(&mut app, Input::Intent(Intent::Action(Action::Send)));
    apply(&mut app, Input::Intent(Intent::Animate));
    assert_eq!(app.view().animation_frame, 1);

    apply(
        &mut app,
        Input::Adapter(AdapterEvent::MessageAdded {
            chat: ChatId(10),
            message: Box::new(MessageView {
                id: MessageId(4),
                sender: "You".to_owned(),
                body: "sending".to_owned(),
                timestamp: "12:01".to_owned(),
                direction: MessageDirection::Outgoing,
                delivery: DeliveryState::Sent,
                reply_to: None,
                details: MessageDetails::default(),
            }),
        }),
    );
    apply(&mut app, Input::Intent(Intent::Animate));
    assert_eq!(app.view().animation_frame, 1);
}

#[test]
fn rpc_acknowledgement_followed_by_its_live_update_keeps_one_message() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
    );
    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
    apply(
        &mut app,
        Input::Intent(Intent::Insert("one copy".to_owned())),
    );
    let sent = app.transition(Input::Intent(Intent::Action(Action::Send)));
    let Some(Effect::SendMessage { local_id, .. }) = sent.effect else {
        panic!("send should expose its optimistic identity")
    };
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::RichMediaAcknowledged {
            chat: ChatId(10),
            local_id,
            server_id: MessageId(4),
        }),
    );

    apply(
        &mut app,
        Input::Adapter(AdapterEvent::MessageAdded {
            chat: ChatId(10),
            message: Box::new(MessageView {
                id: MessageId(4),
                sender: "You".to_owned(),
                body: "one copy".to_owned(),
                timestamp: "12:01".to_owned(),
                direction: MessageDirection::Outgoing,
                delivery: DeliveryState::Sent,
                reply_to: None,
                details: MessageDetails::default(),
            }),
        }),
    );

    assert_eq!(
        app.view()
            .messages
            .iter()
            .filter(|message| message.id == MessageId(4))
            .count(),
        1
    );
}
