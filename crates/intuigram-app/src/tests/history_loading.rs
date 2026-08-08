use super::*;
use crate::TranscriptAnchorView;

#[test]
fn active_history_reports_fresh_and_incremental_effort_until_completion() {
    let mut fresh_app = App::new();
    let bootstrap = fresh_app.transition(Input::Adapter(AdapterEvent::Bootstrap(
        hierarchy_bootstrap(),
    )));
    assert_eq!(bootstrap.view.chat_loading, ChatLoadingState::Idle);
    let selected = fresh_app.transition(Input::Intent(Intent::Action(Action::MoveDown)));
    assert_eq!(selected.view.chat_loading, ChatLoadingState::Fresh);
    let loaded = fresh_app.transition(Input::Adapter(AdapterEvent::ChatLoaded {
        chat: ChatId(20),
        status: None,
        messages: vec![message(20, "fresh")],
        pinned_messages: Vec::new(),
    }));
    assert_eq!(loaded.view.chat_loading, ChatLoadingState::Idle);

    let mut cached_fixture = hierarchy_bootstrap();
    cached_fixture.histories.push(HistoryView {
        chat: ChatId(20),
        thread_root: None,
        messages: vec![message(19, "cached")],
    });
    let mut cached_app = App::new();
    apply(
        &mut cached_app,
        Input::Adapter(AdapterEvent::Bootstrap(cached_fixture)),
    );
    let selected = cached_app.transition(Input::Intent(Intent::Action(Action::MoveDown)));
    assert_eq!(selected.view.chat_loading, ChatLoadingState::Updating);
}

#[test]
fn background_refresh_does_not_replace_a_transcript_being_read() {
    let mut fixture = hierarchy_bootstrap();
    let cached = message(20, "stable cached history");
    fixture.histories.push(HistoryView {
        chat: ChatId(20),
        thread_root: None,
        messages: vec![cached.clone()],
    });
    let mut app = App::new();
    let bootstrap = app.transition(Input::Adapter(AdapterEvent::Bootstrap(fixture)));
    assert_eq!(bootstrap.effect, Some(load_chat(20, None)));
    apply(&mut app, Input::Intent(Intent::Action(Action::MoveDown)));
    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
    apply(
        &mut app,
        Input::Intent(Intent::Action(Action::TargetPreviousMessage)),
    );

    let refreshed = vec![cached.clone(), message(21, "loaded in the background")];
    let loaded = app.transition(Input::Adapter(AdapterEvent::ChatLoaded {
        chat: ChatId(20),
        status: None,
        messages: refreshed.clone(),
        pinned_messages: Vec::new(),
    }));

    assert_eq!(loaded.view.messages, vec![cached]);
    assert_eq!(loaded.view.active_message, Some(0));
    assert!(loaded.view.has_newer_messages);
    assert_jump_adopts_refresh(&mut app, refreshed);
}

#[test]
fn a_live_message_already_returned_by_history_is_not_added_twice() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(hierarchy_bootstrap())),
    );
    let loaded = message(20, "one server message");
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::ChatLoaded {
            chat: ChatId(20),
            status: None,
            messages: vec![loaded.clone()],
            pinned_messages: Vec::new(),
        }),
    );

    let duplicate = app.transition(Input::Adapter(AdapterEvent::MessageAdded {
        chat: ChatId(20),
        message: Box::new(loaded),
    }));

    assert_eq!(duplicate.effect, None);
    apply(&mut app, Input::Intent(Intent::Action(Action::MoveDown)));
    assert_eq!(
        app.view()
            .messages
            .iter()
            .filter(|message| message.id == MessageId(20))
            .count(),
        1
    );
}

#[test]
fn refresh_prunes_stale_cache_without_losing_older_live_or_pending_messages() {
    let mut fixture = hierarchy_bootstrap();
    let mut pending = message(-1, "pending send");
    pending.direction = MessageDirection::Outgoing;
    pending.delivery = DeliveryState::Pending;
    fixture.histories.push(HistoryView {
        chat: ChatId(20),
        thread_root: None,
        messages: vec![
            message(1, "older history"),
            message(8, "stale cache"),
            pending.clone(),
        ],
    });
    let mut app = App::new();

    let bootstrap = app.transition(Input::Adapter(AdapterEvent::Bootstrap(fixture)));
    assert_eq!(bootstrap.effect, Some(load_chat(20, None)));
    let mut live = message(11, "concurrent live update");
    live.direction = MessageDirection::Outgoing;
    live.delivery = DeliveryState::Sent;
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::MessageAdded {
            chat: ChatId(20),
            message: Box::new(live.clone()),
        }),
    );
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::ChatLoaded {
            chat: ChatId(20),
            status: None,
            messages: vec![message(7, "fresh"), message(10, "latest")],
            pinned_messages: Vec::new(),
        }),
    );

    apply(&mut app, Input::Intent(Intent::Action(Action::MoveDown)));
    assert_eq!(
        app.view()
            .messages
            .iter()
            .map(|message| (message.id, message.body.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (MessageId(1), "older history"),
            (MessageId(7), "fresh"),
            (MessageId(10), "latest"),
            (MessageId(11), "concurrent live update"),
            (MessageId(-1), "pending send"),
        ]
    );
}

#[test]
fn rapid_navigation_does_not_drop_an_inactive_background_history() {
    let mut fixture = hierarchy_bootstrap();
    let mut third = fixture.chats[1].clone();
    third.id = ChatId(30);
    third.title = "Compio".to_owned();
    let mut fourth = third.clone();
    fourth.id = ChatId(40);
    fourth.title = "Ratatui".to_owned();
    fixture.chats.extend([third, fourth]);
    let mut app = App::new();

    let bootstrap = app.transition(Input::Adapter(AdapterEvent::Bootstrap(fixture)));
    assert_eq!(bootstrap.effect, Some(load_chat(20, None)));
    for _ in 0..3 {
        apply(&mut app, Input::Intent(Intent::Action(Action::MoveDown)));
    }
    let latest = app.transition(Input::Adapter(AdapterEvent::ChatLoaded {
        chat: ChatId(20),
        status: None,
        messages: Vec::new(),
        pinned_messages: Vec::new(),
    }));
    assert_eq!(latest.effect, Some(load_chat(40, Some(40))));

    let resumed_background = app.transition(Input::Adapter(AdapterEvent::ChatLoaded {
        chat: ChatId(40),
        status: None,
        messages: Vec::new(),
        pinned_messages: Vec::new(),
    }));
    assert_eq!(resumed_background.effect, Some(load_chat(30, None)));
}

#[test]
fn background_thread_refresh_does_not_replace_a_transcript_being_read() {
    let mut fixture = bootstrap();
    let root_messages = fixture.messages.clone();
    let cached = message(30, "stable thread history");
    fixture.histories.push(HistoryView {
        chat: ChatId(10),
        thread_root: Some(MessageId(3)),
        messages: vec![cached.clone()],
    });
    let mut app = App::new();
    apply(&mut app, Input::Adapter(AdapterEvent::Bootstrap(fixture)));
    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::ChatLoaded {
            chat: ChatId(10),
            status: None,
            messages: root_messages,
            pinned_messages: Vec::new(),
        }),
    );
    apply(&mut app, Input::Intent(Intent::Action(Action::JumpLatest)));
    let opened = app.transition(Input::Intent(Intent::Action(Action::OpenThread)));
    assert_eq!(
        opened.effect,
        Some(Effect::LoadThread {
            chat: ChatId(10),
            root: MessageId(3),
        })
    );
    apply(
        &mut app,
        Input::Intent(Intent::Action(Action::TargetPreviousMessage)),
    );

    let refreshed = vec![cached.clone(), message(31, "loaded in the background")];
    let loaded = app.transition(Input::Adapter(AdapterEvent::ThreadLoaded {
        chat: ChatId(10),
        root: MessageId(3),
        messages: refreshed.clone(),
    }));

    assert_eq!(loaded.view.messages, vec![cached]);
    assert_eq!(loaded.view.active_message, Some(0));
    assert!(loaded.view.has_newer_messages);
    assert_jump_adopts_refresh(&mut app, refreshed);
}

#[test]
fn thread_read_is_emitted_after_remaining_background_history() {
    let mut fixture = hierarchy_bootstrap();
    let mut third = fixture.chats[1].clone();
    third.id = ChatId(30);
    third.title = "Compio".to_owned();
    fixture.chats.push(third);
    let mut app = App::new();
    let bootstrap = app.transition(Input::Adapter(AdapterEvent::Bootstrap(fixture)));
    assert_eq!(bootstrap.effect, Some(load_chat(20, None)));

    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
    apply(&mut app, Input::Intent(Intent::Action(Action::JumpLatest)));
    apply(&mut app, Input::Intent(Intent::Action(Action::OpenThread)));
    let foreground = app.transition(Input::Adapter(AdapterEvent::ChatLoaded {
        chat: ChatId(20),
        status: None,
        messages: Vec::new(),
        pinned_messages: Vec::new(),
    }));
    assert_eq!(
        foreground.effect,
        Some(Effect::LoadThread {
            chat: ChatId(10),
            root: MessageId(3),
        })
    );

    let warmup = app.transition(Input::Adapter(AdapterEvent::ThreadLoaded {
        chat: ChatId(10),
        root: MessageId(3),
        messages: vec![message(31, "visible incoming thread message")],
    }));
    assert_eq!(
        warmup.effect,
        Some(Effect::LoadChat {
            chat: ChatId(30),
            selection: None,
            transcript_anchors: vec![TranscriptAnchorView {
                chat: ChatId(10),
                thread: None,
                message: MessageId(3),
            }],
        })
    );

    let read = app.transition(Input::Adapter(AdapterEvent::ChatLoaded {
        chat: ChatId(30),
        status: None,
        messages: Vec::new(),
        pinned_messages: Vec::new(),
    }));
    assert_eq!(
        read.effect,
        Some(Effect::ReadThread {
            chat: ChatId(10),
            root: MessageId(3),
            max_id: MessageId(31),
        })
    );
}

fn message(id: i64, body: &str) -> MessageView {
    MessageView {
        id: MessageId(id),
        sender: "Ferris".to_owned(),
        body: body.to_owned(),
        timestamp: "12:30".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: None,
        details: MessageDetails::default(),
    }
}

fn load_chat(chat: i64, selected_chat: Option<i64>) -> Effect {
    Effect::LoadChat {
        chat: ChatId(chat),
        selection: selected_chat.map(|chat| SelectionView {
            folder: 0,
            chat: Some(ChatId(chat)),
            message: None,
        }),
        transcript_anchors: Vec::new(),
    }
}

fn assert_jump_adopts_refresh(app: &mut App, refreshed: Vec<MessageView>) {
    let jumped = app.transition(Input::Intent(Intent::Action(Action::JumpLatest)));
    assert_eq!(jumped.view.messages, refreshed);
    assert_eq!(jumped.view.active_message, Some(1));
    assert!(!jumped.view.has_newer_messages);
}
