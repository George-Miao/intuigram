#[test]
fn failed_optimistic_send_restores_the_draft_and_marks_the_message_failed() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
    );
    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
    apply(
        &mut app,
        Input::Intent(Intent::Insert("retry me".to_owned())),
    );

    let sent = app.transition(Input::Intent(Intent::Action(Action::Send)));
    let local_id = match sent.effect {
        Some(Effect::SendMessage { local_id, .. }) => local_id,
        effect => panic!("expected optimistic send effect, got {effect:?}"),
    };
    assert!(sent.view.composer.text.is_empty());
    assert_eq!(
        sent.view.messages.last().map(|message| message.delivery),
        Some(DeliveryState::Pending)
    );

    let failed = app.transition(Input::Adapter(AdapterEvent::MessageFailed {
        chat: ChatId(10),
        local_id,
        thread_root: None,
        text: "retry me".to_owned(),
        reason: "Telegram is unavailable".to_owned(),
    }));

    assert_eq!(failed.view.composer.text, "retry me");
    assert_eq!(
        failed.view.notice.as_deref(),
        Some("Telegram is unavailable")
    );
    assert_eq!(
        failed.view.messages.last().map(|message| message.delivery),
        Some(DeliveryState::Failed)
    );
    assert_eq!(
        failed.effect,
        Some(Effect::SaveDraft {
            chat: ChatId(10),
            thread_root: None,
            text: "retry me".to_owned(),
            reply_to: None,
        })
    );
}
#[test]
fn thread_navigation_preserves_parent_history_and_an_independent_draft() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
    );
    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::ChatLoaded {
            chat: ChatId(10),
            messages: bootstrap().messages,
            pinned_messages: Vec::new(),
        }),
    );
    apply(
        &mut app,
        Input::Intent(Intent::Action(Action::TargetPreviousMessage)),
    );
    let opened = app.transition(Input::Intent(Intent::Action(Action::OpenThread)));
    assert_eq!(
        opened.effect,
        Some(Effect::LoadThread {
            chat: ChatId(10),
            root: MessageId(3),
        })
    );
    assert_eq!(opened.view.active_thread, Some(MessageId(3)));
    assert!(opened.view.messages.is_empty());
    let thread_message = MessageView {
        id: MessageId(4),
        sender: "Lin".to_owned(),
        body: "thread reply".to_owned(),
        timestamp: "12:03".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: Some(MessageId(3)),
        details: super::MessageDetails {
            thread_root: Some(MessageId(3)),
            ..super::MessageDetails::default()
        },
    };
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::ThreadLoaded {
            chat: ChatId(10),
            root: MessageId(3),
            messages: vec![thread_message],
        }),
    );
    apply(
        &mut app,
        Input::Intent(Intent::Insert("thread draft".to_owned())),
    );
    apply(&mut app, Input::Intent(Intent::Action(Action::Cancel)));
    let parent = app.view();
    assert_eq!(parent.active_thread, None);
    assert_eq!(parent.messages.len(), 3);
    assert!(parent.composer.text.is_empty());

    apply(&mut app, Input::Intent(Intent::Action(Action::JumpLatest)));
    apply(&mut app, Input::Intent(Intent::Action(Action::OpenThread)));
    assert_eq!(app.view().composer.text, "thread draft");
    assert_eq!(app.view().messages.len(), 1);
}

#[test]
fn chat_movement_changes_active_chat_and_preserves_each_draft() {
    let mut app = App::new();
    let background = app.transition(Input::Adapter(AdapterEvent::Bootstrap(
        hierarchy_bootstrap(),
    )));
    assert_eq!(
        background.effect,
        Some(Effect::LoadChat { chat: ChatId(20) })
    );
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::ChatLoaded {
            chat: ChatId(20),
            messages: Vec::new(),
            pinned_messages: Vec::new(),
        }),
    );
    let opened = app.transition(Input::Intent(Intent::Action(Action::Open)));
    assert_eq!(opened.view.focus, Focus::Composer);
    assert_eq!(opened.effect, Some(Effect::LoadChat { chat: ChatId(10) }));
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::ChatLoaded {
            chat: ChatId(10),
            messages: hierarchy_bootstrap().messages,
            pinned_messages: Vec::new(),
        }),
    );
    apply(
        &mut app,
        Input::Intent(Intent::Insert("first draft".to_owned())),
    );
    apply(&mut app, Input::Intent(Intent::Action(Action::Cancel)));
    let second = app.transition(Input::Intent(Intent::Action(Action::MoveDown)));
    assert_eq!(second.view.active_chat, Some(1));
    assert!(second.view.messages.is_empty());
    assert!(second.view.composer.text.is_empty());
    assert_eq!(second.effect, None);
    let first = app.transition(Input::Intent(Intent::Action(Action::MoveUp)));
    assert_eq!(first.view.active_chat, Some(0));
    assert_eq!(first.view.messages, hierarchy_bootstrap().messages);
    assert_eq!(first.view.composer.text, "first draft");
    assert_eq!(first.effect, None);
}

#[test]
fn revisiting_a_loaded_chat_renders_cached_history_while_refreshing() {
    let mut app = App::new();
    let initial = hierarchy_bootstrap().messages;
    let background = app.transition(Input::Adapter(AdapterEvent::Bootstrap(
        hierarchy_bootstrap(),
    )));
    assert_eq!(
        background.effect,
        Some(Effect::LoadChat { chat: ChatId(20) })
    );

    let second = app.transition(Input::Intent(Intent::Action(Action::MoveDown)));
    assert!(second.view.messages.is_empty());
    assert_eq!(second.effect, None);

    let second_history = vec![MessageView {
        id: MessageId(20),
        sender: "Ferris".to_owned(),
        body: "cached second chat".to_owned(),
        timestamp: "12:20".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: None,
        details: super::MessageDetails::default(),
    }];
    let loaded = app.transition(Input::Adapter(AdapterEvent::ChatLoaded {
        chat: ChatId(20),
        messages: second_history.clone(),
        pinned_messages: Vec::new(),
    }));
    assert_eq!(loaded.view.messages, second_history);

    let first = app.transition(Input::Intent(Intent::Action(Action::MoveUp)));
    assert_eq!(first.view.messages, initial);
    assert_eq!(first.effect, Some(Effect::LoadChat { chat: ChatId(10) }));

    let mut refreshed = hierarchy_bootstrap().messages;
    refreshed.push(MessageView {
        id: MessageId(4),
        sender: "Lin".to_owned(),
        body: "arrived while away".to_owned(),
        timestamp: "12:21".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Sent,
        reply_to: None,
        details: super::MessageDetails::default(),
    });
    let refreshed_view = app.transition(Input::Adapter(AdapterEvent::ChatLoaded {
        chat: ChatId(10),
        messages: refreshed.clone(),
        pinned_messages: Vec::new(),
    }));
    assert_eq!(refreshed_view.view.messages, refreshed);
}

#[test]
fn bootstrap_cached_history_renders_before_a_background_refresh() {
    let mut fixture = hierarchy_bootstrap();
    let cached = MessageView {
        id: MessageId(20),
        sender: "Ferris".to_owned(),
        body: "durable cached history".to_owned(),
        timestamp: "12:20".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: None,
        details: super::MessageDetails::default(),
    };
    fixture.histories.push(super::HistoryView {
        chat: ChatId(20),
        thread_root: None,
        messages: vec![cached.clone()],
    });
    let mut app = App::new();
    let background = app.transition(Input::Adapter(AdapterEvent::Bootstrap(fixture)));
    assert_eq!(
        background.effect,
        Some(Effect::LoadChat { chat: ChatId(20) })
    );

    let switched = app.transition(Input::Intent(Intent::Action(Action::MoveDown)));

    assert_eq!(switched.view.messages, vec![cached]);
    assert_eq!(switched.effect, None);
}

use super::*;
