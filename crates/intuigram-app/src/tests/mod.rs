use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::task::{Context, Poll};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use intuigram_lib::{
    Action, AdapterEvent, App, Bootstrap, ChatId, ChatKind, Effect, Input, Intent, MessageId,
    Update,
};
use intuigram_tui::UiEvent;

use super::runtime::{AdapterBatch, BackendOutput, PendingEffect, append_ready_text};
use super::{
    AdapterEffect, ApplicationAdapterEvents, ApplicationBackend, ApplicationEvents,
    ApplicationExit, ApplicationState, ApplicationUi, AttachmentPayload, AttachmentStore,
    EFFECT_CAPACITY, Error, Result, application_fixture, enqueue_effect, run_application,
    run_application_state,
};

mod accounts;
mod background_progress;
mod cached;
mod connection_retry;
mod effect_fairness;
mod error_shutdown;
mod idle;
mod misc;
mod queue;
mod startup_loading;

fn application_state(bootstrap: Bootstrap) -> (App, Update) {
    let mut app = App::new();
    let update = app.transition(Input::Adapter(AdapterEvent::Bootstrap(bootstrap)));
    (app, update)
}

#[derive(Clone)]
struct PendingHistoryBackend {
    polls: Rc<Cell<usize>>,
}

impl ApplicationBackend for PendingHistoryBackend {
    async fn execute(
        &self,
        effect: AdapterEffect,
        _peers: intuigram_telegram::PeerDirectory,
    ) -> Result<BackendOutput> {
        let Effect::LoadChat { chat, .. } = effect.effect else {
            return Ok(BackendOutput::event(None));
        };
        std::future::poll_fn(|cx| {
            let polls = self.polls.get();
            self.polls.set(polls + 1);
            if polls == 0 {
                cx.waker().wake_by_ref();
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        })
        .await;
        Ok(BackendOutput::event(Some(AdapterEvent::ChatLoaded {
            chat,
            status: None,
            messages: Vec::new(),
            pinned_messages: Vec::new(),
        })))
    }
}

struct RecordingUi {
    views: Rc<RefCell<Vec<intuigram_lib::View>>>,
}

impl ApplicationUi for RecordingUi {
    fn draw(&mut self, view: &intuigram_lib::View) -> intuigram_tui::Result<()> {
        self.views.borrow_mut().push(view.clone());
        Ok(())
    }

    fn resolve_event(&self, _view: &intuigram_lib::View, event: Event) -> Option<UiEvent> {
        let Event::Key(key) = event else {
            return Some(UiEvent::Redraw);
        };
        match key.code {
            KeyCode::Char('o') => Some(UiEvent::Intent(Intent::Action(Action::Open))),
            KeyCode::Char('j') => Some(UiEvent::Intent(Intent::Action(Action::MoveDown))),
            KeyCode::Char('x') => Some(UiEvent::Intent(Intent::Insert("x".to_owned()))),
            KeyCode::Char('q') => Some(UiEvent::Intent(Intent::Action(Action::Quit))),
            _ => None,
        }
    }

    fn poll_redraw(&mut self, _cx: &mut Context<'_>) -> Poll<intuigram_tui::Result<()>> {
        Poll::Pending
    }
}

enum EventStep {
    Ready(Event),
    Pending,
}

struct ScriptedEvents {
    steps: VecDeque<EventStep>,
}

impl ApplicationEvents for ScriptedEvents {
    fn poll_next_event(&mut self, cx: &mut Context<'_>) -> Poll<intuigram_tui::Result<Event>> {
        match self.steps.pop_front().expect("event script should not end") {
            EventStep::Ready(event) => Poll::Ready(Ok(event)),
            EventStep::Pending => {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
}

struct NoAdapterEvents;

impl ApplicationAdapterEvents for NoAdapterEvents {
    fn poll_adapter_event(&mut self, _cx: &mut Context<'_>) -> Poll<Result<AdapterBatch>> {
        Poll::Pending
    }
}

struct OneAdapterBatch {
    batch: Option<AdapterBatch>,
}

impl ApplicationAdapterEvents for OneAdapterBatch {
    fn poll_adapter_event(&mut self, _cx: &mut Context<'_>) -> Poll<Result<AdapterBatch>> {
        self.batch
            .take()
            .map_or(Poll::Pending, |batch| Poll::Ready(Ok(batch)))
    }
}

struct AlwaysPendingEvents;

impl ApplicationEvents for AlwaysPendingEvents {
    fn poll_next_event(&mut self, _cx: &mut Context<'_>) -> Poll<intuigram_tui::Result<Event>> {
        Poll::Pending
    }
}

#[derive(Clone)]
struct PeerAwareBackend {
    chat: ChatId,
    resolved: Rc<Cell<bool>>,
}

impl ApplicationBackend for PeerAwareBackend {
    async fn execute(
        &self,
        effect: AdapterEffect,
        peers: intuigram_telegram::PeerDirectory,
    ) -> Result<BackendOutput> {
        let Effect::LoadChat { chat, .. } = effect.effect else {
            return Ok(BackendOutput::event(None));
        };
        self.resolved.set(chat == self.chat && peers.contains(chat));
        Ok(BackendOutput::event(Some(AdapterEvent::ChatLoaded {
            chat,
            status: None,
            messages: Vec::new(),
            pinned_messages: Vec::new(),
        })))
    }
}

fn key(character: char) -> Event {
    Event::Key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE))
}

#[test]
fn queued_text_events_form_one_input() {
    let terminal = RecordingUi {
        views: Rc::new(RefCell::new(Vec::new())),
    };
    let mut events = ScriptedEvents {
        steps: [
            EventStep::Ready(key('x')),
            EventStep::Ready(key('x')),
            EventStep::Ready(key('q')),
        ]
        .into(),
    };
    let (_, update) = application_state(application_fixture());
    let mut text = "x".to_owned();

    let pending = append_ready_text(&terminal, &mut events, &update.view, &mut text)
        .expect("queued terminal input should resolve");

    assert_eq!(text, "xxx");
    assert!(matches!(
        pending,
        Some(Event::Key(KeyEvent {
            code: KeyCode::Char('q'),
            ..
        }))
    ));
}

#[test]
fn chat_history_loading_does_not_block_terminal_input() {
    let views = Rc::new(RefCell::new(Vec::new()));
    let polls = Rc::new(Cell::new(0));
    let mut terminal = RecordingUi {
        views: Rc::clone(&views),
    };
    let mut events = ScriptedEvents {
        steps: [
            EventStep::Ready(key('o')),
            EventStep::Pending,
            EventStep::Ready(key('x')),
            EventStep::Pending,
            EventStep::Ready(key('q')),
        ]
        .into(),
    };
    let backend = PendingHistoryBackend {
        polls: Rc::clone(&polls),
    };
    let mut fixture = application_fixture();
    fixture.chats[0].kind = ChatKind::Private;
    let mut adapter_events = NoAdapterEvents;
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");

    runtime
        .block_on(run_application(
            &mut terminal,
            &mut events,
            &mut adapter_events,
            backend,
            intuigram_telegram::PeerDirectory::default(),
            fixture,
        ))
        .expect("application should stop cleanly");

    assert!(polls.get() >= 2, "history future should make progress");
    assert!(
        views.borrow().iter().any(|view| view.composer.text == "x"),
        "terminal input should update the Draft while history is pending"
    );
}

#[derive(Clone)]
struct ShutdownReleasesHistoryBackend {
    shutdown: Rc<Cell<bool>>,
}

impl ApplicationBackend for ShutdownReleasesHistoryBackend {
    async fn execute(
        &self,
        effect: AdapterEffect,
        _peers: intuigram_telegram::PeerDirectory,
    ) -> Result<BackendOutput> {
        let Effect::LoadChat { chat, .. } = effect.effect else {
            return Ok(BackendOutput::event(None));
        };
        std::future::poll_fn(|cx| {
            if self.shutdown.get() {
                Poll::Ready(())
            } else {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        })
        .await;
        Ok(BackendOutput::event(Some(AdapterEvent::ChatLoaded {
            chat,
            status: None,
            messages: Vec::new(),
            pinned_messages: Vec::new(),
        })))
    }

    fn begin_shutdown(&self) {
        self.shutdown.set(true);
    }
}

#[test]
fn rapid_chat_switches_draw_each_loading_selection_before_history_finishes() {
    let views = Rc::new(RefCell::new(Vec::new()));
    let mut terminal = RecordingUi {
        views: Rc::clone(&views),
    };
    let mut events = ScriptedEvents {
        steps: [
            EventStep::Ready(key('j')),
            EventStep::Ready(key('j')),
            EventStep::Ready(key('q')),
        ]
        .into(),
    };
    let backend = ShutdownReleasesHistoryBackend {
        shutdown: Rc::new(Cell::new(false)),
    };
    let mut adapter_events = NoAdapterEvents;
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");

    runtime
        .block_on(run_application(
            &mut terminal,
            &mut events,
            &mut adapter_events,
            backend,
            intuigram_telegram::PeerDirectory::default(),
            application_fixture(),
        ))
        .expect("application should stop cleanly");

    let views = views.borrow();
    for active in [1, 2] {
        assert!(
            views.iter().any(|view| {
                view.active_chat == Some(active)
                    && view.chat_loading != intuigram_lib::ChatLoadingState::Idle
            }),
            "Chat {active} should render its loading state during rapid navigation"
        );
    }
}

#[test]
fn live_peer_directory_reaches_the_next_chat_operation() {
    let target = ChatId(206_899_663);
    let mut fixture = application_fixture();
    let mut discovered = fixture.chats.remove(1);
    discovered.id = target;
    fixture.chats.truncate(1);
    let mut peers = intuigram_telegram::PeerDirectory::default();
    peers.insert(intuigram_telegram::PeerAddress::User {
        id: target.0,
        access_hash: 11,
    });
    let resolved = Rc::new(Cell::new(false));
    let backend = PeerAwareBackend {
        chat: target,
        resolved: Rc::clone(&resolved),
    };
    let mut terminal = RecordingUi {
        views: Rc::new(RefCell::new(Vec::new())),
    };
    let mut events = ScriptedEvents {
        steps: [
            EventStep::Ready(key('j')),
            EventStep::Pending,
            EventStep::Pending,
            EventStep::Ready(key('q')),
        ]
        .into(),
    };
    let mut adapter_events = OneAdapterBatch {
        batch: Some(AdapterBatch {
            event: Some(AdapterEvent::ChatDiscovered { chat: discovered }),
            peers,
        }),
    };
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");

    runtime
        .block_on(run_application(
            &mut terminal,
            &mut events,
            &mut adapter_events,
            backend,
            intuigram_telegram::PeerDirectory::default(),
            fixture,
        ))
        .expect("application should stop cleanly");

    assert!(
        resolved.get(),
        "the operation must receive peers learned before the Chat became active"
    );
}

#[test]
fn reconnect_handoff_preserves_attachment_payloads_and_ids() {
    let mut disconnected = AttachmentStore::default();
    let first = disconnected.register(AttachmentPayload::Image {
        mime_type: "image/png".to_owned(),
        bytes: vec![1, 2, 3],
    });
    let mut connected = AttachmentStore::default();

    connected.merge(disconnected);
    let second = connected.register(AttachmentPayload::Image {
        mime_type: "image/png".to_owned(),
        bytes: vec![4, 5, 6],
    });

    assert!(connected.payloads.contains_key(&first));
    assert!(second.0 > first.0);
}
