use std::cell::Cell;
use std::rc::Rc;
use std::task::Poll;

use intuigram_app::ConnectionState;

use super::super::account_loading::wait_for_account_load;
use super::super::{AccountSessionExit, Loading};
use super::{AlwaysPendingEvents, EventStep, RecordingUi, ScriptedEvents, key};

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
