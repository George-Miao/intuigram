use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::io;
use std::path::PathBuf;
use std::rc::Rc;
use std::task::{Context, Poll};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use intuigram_app::{
    Action, AdapterEvent, ChatId, ChatKind, DeliveryState, Effect, Intent, MediaCard, MediaKind,
    MessageDetails, MessageDirection, MessageId, MessageView, TextEntity, TextEntityKind,
};
use intuigram_store::{CachedAccount, StoredChat, StoredDraft, StoredFolder};
use intuigram_telegram::{LoginCodeDelivery, LoginCodeDeliveryMethod};
use intuigram_tui::UiEvent;

use super::runtime_adapters::AdapterBatch;
use super::{
    AdapterEffect, ApplicationAdapterEvents, ApplicationBackend, ApplicationEvents,
    ApplicationExit, ApplicationState, ApplicationUi, AttachmentPayload, AttachmentStore,
    EFFECT_CAPACITY, Error, PRIMARY_DC_ENDPOINT, PendingEffect, Result, application_fixture,
    cached_bootstrap, connection_failure_reason, encode_stored_message, enqueue_effect,
    error_lines, login_code_delivery_message, login_code_delivery_method_name, parse_arguments,
    run_application, run_application_state, seconds_until_at,
};

struct PendingHistoryBackend {
    polls: Rc<Cell<usize>>,
}

#[test]
fn cached_account_restores_rich_thread_history_and_drafts() {
    let message = MessageView {
        id: MessageId(42),
        sender: "Ada".to_owned(),
        body: "cached caption".to_owned(),
        timestamp: "12:00".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: Some(MessageId(40)),
        details: MessageDetails {
            entities: vec![TextEntity {
                offset: 0,
                length: 6,
                kind: TextEntityKind::Bold,
            }],
            media: Some(MediaCard {
                kind: MediaKind::Photo,
                title: "Photo".to_owned(),
                description: "image".to_owned(),
                details: Vec::new(),
                poll: None,
                remote_id: Some("99".to_owned()),
            }),
            thread_root: Some(MessageId(41)),
            ..MessageDetails::default()
        },
    };
    let cached = CachedAccount {
        cursors: Vec::new(),
        folders: vec![StoredFolder {
            id: 0,
            title: "All".to_owned(),
            unread: 1,
        }],
        chats: vec![StoredChat {
            id: 7,
            kind: "private".to_owned(),
            title: "Ada".to_owned(),
            preview: "cached caption".to_owned(),
            unread: 1,
            pinned: false,
            folders: vec![0],
        }],
        messages: vec![encode_stored_message(ChatId(7), &message)],
        drafts: vec![StoredDraft {
            chat_id: 7,
            thread_root: Some(41),
            text: "cached Draft".to_owned(),
            reply_to: Some(42),
            modified_at: 10,
        }],
    };

    let bootstrap = cached_bootstrap("Ada".to_owned(), cached);

    assert_eq!(bootstrap.chats[0].kind, ChatKind::Private);
    assert_eq!(bootstrap.histories[0].thread_root, Some(MessageId(41)));
    assert_eq!(bootstrap.histories[0].messages, vec![message]);
    assert_eq!(bootstrap.drafts[0].text, "cached Draft");
}

impl ApplicationBackend for PendingHistoryBackend {
    async fn execute(
        &mut self,
        effect: AdapterEffect,
        _peers: intuigram_telegram::PeerDirectory,
    ) -> Result<Option<AdapterEvent>> {
        let Effect::LoadChat { chat } = effect.effect else {
            return Ok(None);
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
        Ok(Some(AdapterEvent::ChatLoaded {
            chat,
            messages: Vec::new(),
        }))
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

struct PeerAwareBackend {
    chat: ChatId,
    resolved: Rc<Cell<bool>>,
}

impl ApplicationBackend for PeerAwareBackend {
    async fn execute(
        &mut self,
        effect: AdapterEffect,
        peers: intuigram_telegram::PeerDirectory,
    ) -> Result<Option<AdapterEvent>> {
        let Effect::LoadChat { chat } = effect.effect else {
            return Ok(None);
        };
        self.resolved.set(chat == self.chat && peers.contains(chat));
        Ok(Some(AdapterEvent::ChatLoaded {
            chat,
            messages: Vec::new(),
        }))
    }
}

struct AlwaysPendingEvents;

impl ApplicationEvents for AlwaysPendingEvents {
    fn poll_next_event(&mut self, _cx: &mut Context<'_>) -> Poll<intuigram_tui::Result<Event>> {
        Poll::Pending
    }
}

struct FailingConnectionBackend {
    observed: Rc<RefCell<Vec<AdapterEffect>>>,
}

impl ApplicationBackend for FailingConnectionBackend {
    async fn execute(
        &mut self,
        effect: AdapterEffect,
        _peers: intuigram_telegram::PeerDirectory,
    ) -> Result<Option<AdapterEvent>> {
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
    let active = None::<PendingEffect<PendingHistoryBackend>>;

    let error = enqueue_effect(&mut pending, &active, Some(Effect::Reconnect))
        .expect_err("a saturated effect queue should be reported");

    assert!(matches!(error, Error::EffectsFull { .. }));
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

#[test]
fn bootstrap_uses_the_production_dc_2_endpoint() {
    assert_eq!(PRIMARY_DC_ENDPOINT.to_string(), "149.154.167.41:443");
}

#[test]
fn qr_expiry_uses_the_telegram_server_time_offset() {
    assert_eq!(seconds_until_at(1_030, 1_000, 10), 20);
    assert_eq!(seconds_until_at(1_030, 1_000, 40), 0);
}

#[test]
fn telegram_app_login_codes_name_the_actual_destination() {
    assert_eq!(
        login_code_delivery_message(&LoginCodeDelivery::TelegramApp { length: 5 }),
        "Telegram sent a 5-digit code to the Telegram app on another logged-in device."
    );
}

#[test]
fn login_code_fallback_names_sms_delivery() {
    assert_eq!(
        login_code_delivery_method_name(LoginCodeDeliveryMethod::Sms),
        "SMS delivery"
    );
}

#[test]
fn command_line_paths_are_parsed_and_the_obsolete_demo_flag_is_rejected() {
    let parsed = parse_arguments([
        "--data-dir".to_owned(),
        "/tmp/intuigram-data".to_owned(),
        "--cache-dir".to_owned(),
        "/tmp/intuigram-cache".to_owned(),
    ])
    .expect("valid command line should parse");

    assert_eq!(
        parsed.data.expect("data override should exist"),
        PathBuf::from("/tmp/intuigram-data")
    );
    assert_eq!(
        parsed.cache.expect("cache override should exist"),
        PathBuf::from("/tmp/intuigram-cache")
    );
    assert!(parse_arguments(["--demo".to_owned()]).is_err());
}

#[test]
fn errors_are_rendered_one_line_per_source_layer() {
    let error = Error::Runtime {
        source: io::Error::other("driver setup\nfailed"),
    };

    assert_eq!(
        error_lines(&error),
        [
            "failed to initialize the Compio runtime",
            "driver setup failed"
        ]
    );
}

#[test]
fn synchronization_gap_enters_the_reconnect_path() {
    let error = Error::CommitTelegramUpdate {
        source: intuigram::SyncError::UpdateGap {
            scope: "channel:-1000000000005".to_owned(),
        },
    };

    assert!(connection_failure_reason(&error).is_some());
}
