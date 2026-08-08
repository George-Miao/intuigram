use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::task::{Context, Poll};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use intuigram_app::{Action, AdapterEvent, ChatId, Effect, Intent, MessageId};
use intuigram_tui::UiEvent;

use super::runtime::{AdapterBatch, BackendOutput, PendingEffect};
use super::{
    AdapterEffect, ApplicationAdapterEvents, ApplicationBackend, ApplicationEvents,
    ApplicationExit, ApplicationState, ApplicationUi, AttachmentPayload, AttachmentStore,
    EFFECT_CAPACITY, Error, Result, application_fixture, enqueue_effect, run_application,
    run_application_state,
};

mod accounts;
mod cached;
mod effect_fairness;
mod misc;

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
    views: Rc<RefCell<Vec<intuigram_app::View>>>,
}

impl ApplicationUi for RecordingUi {
    fn draw(&mut self, view: &intuigram_app::View) -> intuigram_tui::Result<()> {
        self.views.borrow_mut().push(view.clone());
        Ok(())
    }

    fn resolve_event(&self, _view: &intuigram_app::View, event: Event) -> Option<UiEvent> {
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

struct AlwaysPendingEvents;

impl ApplicationEvents for AlwaysPendingEvents {
    fn poll_next_event(&mut self, _cx: &mut Context<'_>) -> Poll<intuigram_tui::Result<Event>> {
        Poll::Pending
    }
}

#[derive(Clone)]
struct FailingConnectionBackend {
    observed: Rc<RefCell<Vec<AdapterEffect>>>,
}

impl ApplicationBackend for FailingConnectionBackend {
    async fn execute(
        &self,
        effect: AdapterEffect,
        _peers: intuigram_telegram::PeerDirectory,
    ) -> Result<BackendOutput> {
        self.observed.borrow_mut().push(effect);
        Err(Error::TelegramUpdatesClosed)
    }
}

fn key(character: char) -> Event {
    Event::Key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE))
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

    assert!(polls.get() >= 2, "history future should make progress");
    assert!(
        views.borrow().iter().any(|view| view.composer.text == "x"),
        "terminal input should update the Draft while history is pending"
    );
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
fn a_full_effect_queue_fails_instead_of_blocking_terminal_input() {
    let mut pending = VecDeque::from(vec![
        AdapterEffect {
            effect: Effect::Reconnect,
            random_id: None,
        };
        EFFECT_CAPACITY
    ]);
    let active = futures_util::stream::FuturesUnordered::<PendingEffect>::new();

    let error = enqueue_effect(&mut pending, &active, &[], Some(Effect::Reconnect))
        .expect_err("a saturated effect queue should be reported");

    assert!(matches!(error, Error::EffectsFull { .. }));
}

#[test]
fn rapid_selection_saves_keep_only_the_latest_request() {
    let active = futures_util::stream::FuturesUnordered::<PendingEffect>::new();
    let mut pending = VecDeque::new();

    for chat in 1..=(EFFECT_CAPACITY as i64 + 1) {
        enqueue_effect(
            &mut pending,
            &active,
            &[],
            Some(Effect::SaveSelection {
                folder: 0,
                chat: Some(ChatId(chat)),
                message: None,
                transcript_anchors: Vec::new(),
            }),
        )
        .expect("rapid Chat navigation should coalesce durable selection writes");
    }

    assert_eq!(pending.len(), 1);
    assert!(matches!(
        &pending[0].effect,
        Effect::SaveSelection {
            chat: Some(ChatId(chat)),
            ..
        } if *chat == EFFECT_CAPACITY as i64 + 1
    ));
}

#[test]
fn connection_failure_returns_the_same_send_for_retry() {
    let views = Rc::new(RefCell::new(Vec::new()));
    let observed = Rc::new(RefCell::new(Vec::new()));
    let mut terminal = RecordingUi {
        views: Rc::clone(&views),
    };
    let mut events = AlwaysPendingEvents;
    let mut adapter_events = NoAdapterEvents;
    let backend = FailingConnectionBackend {
        observed: Rc::clone(&observed),
    };
    let mut app = intuigram_app::App::new();
    let update = app.transition(intuigram_app::Input::Adapter(AdapterEvent::Bootstrap(
        application_fixture(),
    )));
    let send = AdapterEffect::new(Effect::SendMessage {
        chat: ChatId(10),
        text: "retry once connected".to_owned(),
        entities: Vec::new(),
        link_preview: true,
        reply_to: None,
        thread_root: None,
        attachments: Vec::new(),
        local_id: MessageId(-1),
    })
    .expect("operation id should be generated");
    let expected_random_id = send.random_id;
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");

    let exit = runtime
        .block_on(run_application_state(
            &mut terminal,
            &mut events,
            &mut adapter_events,
            backend,
            ApplicationState {
                app,
                update,
                pending_effects: VecDeque::from([send]),
                peers: intuigram_telegram::PeerDirectory::default(),
            },
        ))
        .expect("connection failure should become an application handoff");
    let ApplicationExit::Disconnected(state) = exit else {
        panic!("connection failure should not quit")
    };

    assert_eq!(observed.borrow().len(), 1);
    assert_eq!(state.pending_effects.len(), 2);
    assert_eq!(state.pending_effects[0].random_id, expected_random_id);
    assert_eq!(
        state.app.view().connection,
        intuigram_app::ConnectionState::ReconnectCooldown
    );
    assert!(
        views
            .borrow()
            .iter()
            .any(|view| { view.connection == intuigram_app::ConnectionState::ReconnectCooldown }),
        "the TUI should render the disconnect before reconnecting"
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
