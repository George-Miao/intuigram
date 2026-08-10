use std::cell::Cell;
use std::rc::Rc;
use std::task::Poll;

use intuigram_lib::{Action, ChatKind, ConnectionState, Input, Intent};

use super::super::account_loading::wait_for_account_load;
use super::super::runtime::wait_for_reconnect_cleanup;
use super::super::{AccountSessionExit, Loading};
use super::{
    AlwaysPendingEvents, EventStep, RecordingUi, ScriptedEvents, application_fixture, key,
};

#[test]
fn pending_account_load_keeps_terminal_input_responsive() {
    let views = Rc::new(std::cell::RefCell::new(Vec::new()));
    let polls = Rc::new(Cell::new(0));
    let mut terminal = RecordingUi {
        views: Rc::clone(&views),
    };
    let mut events = ScriptedEvents {
        steps: [EventStep::Pending, EventStep::Ready(key('q'))].into(),
    };
    let load_polls = Rc::clone(&polls);
    let load = std::future::poll_fn(move |_cx| {
        load_polls.set(load_polls.get() + 1);
        Poll::<()>::Pending
    });
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");

    let outcome = runtime
        .block_on(wait_for_account_load(
            &mut terminal,
            &mut events,
            "Ada".to_owned(),
            "telegram:7".to_owned(),
            Vec::new(),
            load,
        ))
        .expect("loading UI should stop cleanly");

    assert!(matches!(outcome, Loading::Exit(AccountSessionExit::Quit)));
    assert!(polls.get() > 0, "account load future should make progress");
    assert!(
        views.borrow().iter().any(|view| {
            view.connection == ConnectionState::Connecting && view.chats.is_empty()
        })
    );
}

#[test]
fn pending_account_load_advances_the_loading_animation() {
    let views = Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut terminal = RecordingUi {
        views: Rc::clone(&views),
    };
    let mut events = AlwaysPendingEvents;
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");

    let outcome = runtime
        .block_on(wait_for_account_load(
            &mut terminal,
            &mut events,
            "Ada".to_owned(),
            "telegram:7".to_owned(),
            Vec::new(),
            async {
                compio::time::sleep(std::time::Duration::from_millis(190)).await;
            },
        ))
        .expect("loading UI should finish cleanly");

    assert!(matches!(outcome, Loading::Ready(())));
    assert!(
        views.borrow().iter().any(|view| view.animation_frame > 0),
        "pending Account work should advance visible animation frames"
    );
}

#[test]
fn pending_reconnect_cleanup_keeps_terminal_input_responsive() {
    let views = Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut terminal = RecordingUi {
        views: Rc::clone(&views),
    };
    let mut events = ScriptedEvents {
        steps: [
            EventStep::Ready(key('x')),
            EventStep::Pending,
            EventStep::Pending,
        ]
        .into(),
    };
    let mut fixture = application_fixture();
    fixture.chats[0].kind = ChatKind::Private;
    let (mut app, _) = crate::Application::new(fixture).into_parts();
    let mut update = app.transition(Input::Intent(Intent::Action(Action::Open)));
    let mut pending_effects = std::collections::VecDeque::new();
    let polls = Rc::new(Cell::new(0));
    let operation_polls = Rc::clone(&polls);
    let cleanup = std::future::poll_fn(move |_cx| {
        let polls = operation_polls.get();
        operation_polls.set(polls + 1);
        if polls == 0 {
            Poll::Pending
        } else {
            Poll::Ready(Ok(()))
        }
    });
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");

    let outcome = runtime
        .block_on(wait_for_reconnect_cleanup(
            &mut terminal,
            &mut events,
            &mut app,
            &mut update,
            &mut pending_effects,
            cleanup,
        ))
        .expect("reconnect cleanup should finish cleanly");

    assert!(matches!(outcome, Loading::Ready(())));
    assert!(polls.get() >= 2, "reconnect cleanup should make progress");
    assert!(
        views.borrow().iter().any(|view| view.composer.text == "x"),
        "terminal input should update the Draft while reconnect cleanup is pending"
    );
}
