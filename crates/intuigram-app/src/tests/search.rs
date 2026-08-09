use super::*;

#[test]
fn search_scope_and_reply_send_follow_current_context() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
    );
    let search = app.transition(Input::Intent(Intent::Action(Action::Search)));
    assert_eq!(
        search.view.search.expect("search should be open").scope,
        SearchScope::Account
    );
    for action in [
        Action::Cancel,
        Action::Open,
        Action::TargetPreviousMessage,
        Action::Reply,
    ] {
        apply(&mut app, Input::Intent(Intent::Action(action)));
    }
    apply(&mut app, Input::Intent(Intent::Insert("hello".to_owned())));
    apply(&mut app, Input::Intent(Intent::Action(Action::Newline)));
    apply(&mut app, Input::Intent(Intent::Insert("world".to_owned())));
    let sent = app.transition(Input::Intent(Intent::Action(Action::Send)));
    assert_eq!(
        sent.effect,
        Some(Effect::SendMessage {
            chat: ChatId(10),
            text: "hello\nworld".to_owned(),
            entities: Vec::new(),
            link_preview: true,
            reply_to: Some(MessageId(3)),
            thread_root: None,
            attachments: Vec::new(),
            local_id: MessageId(-1),
        })
    );
    assert_eq!(
        sent.view.messages.last().map(|message| message.delivery),
        Some(DeliveryState::Pending)
    );
    assert_eq!(sent.view.focus, Focus::Composer);
}
