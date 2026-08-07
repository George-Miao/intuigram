use super::*;

#[test]
fn pointer_targets_follow_the_existing_interaction_hierarchy() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(hierarchy_bootstrap())),
    );

    let folder = app.transition(Input::Intent(Intent::Activate(ActivationTarget::Folder(1))));
    assert_eq!(folder.view.active_folder, 1);
    assert_eq!(folder.view.focus, Focus::Chats);
    assert_eq!(folder.view.chats[0].id, ChatId(20));

    apply(
        &mut app,
        Input::Intent(Intent::Activate(ActivationTarget::Folder(0))),
    );
    let chat = app.transition(Input::Intent(Intent::Activate(ActivationTarget::Chat(
        ChatId(20),
    ))));
    assert_eq!(chat.view.active_chat, Some(1));
    assert_eq!(chat.view.focus, Focus::Chats);

    apply(
        &mut app,
        Input::Intent(Intent::Activate(ActivationTarget::Chat(ChatId(10)))),
    );
    apply(
        &mut app,
        Input::Intent(Intent::Activate(ActivationTarget::Composer)),
    );
    assert_eq!(app.view().focus, Focus::Composer);

    apply(
        &mut app,
        Input::Intent(Intent::Activate(ActivationTarget::Message(MessageId(2)))),
    );
    assert_eq!(app.view().focus, Focus::Transcript);
    assert_eq!(app.view().active_message, Some(1));
}

#[test]
fn stale_pointer_targets_do_not_change_the_current_selection() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
    );
    let before = app.view();

    apply(
        &mut app,
        Input::Intent(Intent::Activate(ActivationTarget::Chat(ChatId(999)))),
    );
    apply(
        &mut app,
        Input::Intent(Intent::Activate(ActivationTarget::Message(MessageId(999)))),
    );

    assert_eq!(app.view().active_chat, before.active_chat);
    assert_eq!(app.view().active_message, before.active_message);
}

#[test]
fn message_selection_is_cleared_when_its_chat_or_message_disappears() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(hierarchy_bootstrap())),
    );
    apply(
        &mut app,
        Input::Intent(Intent::Activate(ActivationTarget::Message(MessageId(2)))),
    );
    apply(
        &mut app,
        Input::Intent(Intent::Action(Action::ToggleMessageSelection)),
    );
    assert_eq!(app.view().selected_messages, vec![MessageId(2)]);

    apply(
        &mut app,
        Input::Intent(Intent::Activate(ActivationTarget::Chat(ChatId(20)))),
    );
    assert!(app.view().selected_messages.is_empty());

    apply(
        &mut app,
        Input::Intent(Intent::Activate(ActivationTarget::Chat(ChatId(10)))),
    );
    apply(
        &mut app,
        Input::Intent(Intent::Activate(ActivationTarget::Message(MessageId(2)))),
    );
    apply(
        &mut app,
        Input::Intent(Intent::Action(Action::ToggleMessageSelection)),
    );
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::MessagesDeleted {
            chat: Some(ChatId(10)),
            ids: vec![MessageId(2)],
        }),
    );
    assert!(app.view().selected_messages.is_empty());
}
