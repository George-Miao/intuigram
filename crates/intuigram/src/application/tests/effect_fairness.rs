use std::cell::Cell;
use std::collections::VecDeque;
use std::future::poll_fn;
use std::rc::Rc;
use std::task::{Context, Poll};
use std::time::Duration;

use crossterm::event::Event;
use intuigram_app::{
    AdapterEvent, ChatId, DeliveryState, Effect, MessageDetails, MessageDirection, MessageId,
    MessageView,
};

use super::super::runtime::{AdapterBatch, BackendOutput, notification_key};
use super::super::{
    AdapterEffect, ApplicationAdapterEvents, ApplicationBackend, ApplicationEvents,
    EFFECT_CAPACITY, Result, application_fixture, enqueue_effect, run_application,
};
use super::{RecordingUi, key};

struct BurstAdapterEvents {
    next_message: usize,
    total: usize,
    emitted: Rc<Cell<usize>>,
}

impl ApplicationAdapterEvents for BurstAdapterEvents {
    fn poll_adapter_event(&mut self, _cx: &mut Context<'_>) -> Poll<Result<AdapterBatch>> {
        if self.next_message >= self.total {
            return Poll::Pending;
        }
        self.next_message += 1;
        self.emitted.set(self.next_message);
        Poll::Ready(Ok(AdapterBatch {
            event: Some(AdapterEvent::MessageAdded {
                chat: ChatId(101),
                message: Box::new(MessageView {
                    id: MessageId(self.next_message as i64 + 100),
                    sender: "Lin".to_owned(),
                    body: format!("burst {}", self.next_message),
                    timestamp: "now".to_owned(),
                    direction: MessageDirection::Incoming,
                    delivery: DeliveryState::Sent,
                    reply_to: None,
                    details: MessageDetails::default(),
                }),
            }),
            peers: intuigram_telegram::PeerDirectory::default(),
        }))
    }
}

#[derive(Clone)]
struct CountingBackend {
    notifications: Rc<Cell<usize>>,
}

impl ApplicationBackend for CountingBackend {
    async fn execute(
        &self,
        effect: AdapterEffect,
        _peers: intuigram_telegram::PeerDirectory,
    ) -> Result<BackendOutput> {
        if matches!(effect.effect, Effect::Notify { .. }) {
            self.notifications
                .set(self.notifications.get().saturating_add(1));
        }
        Ok(BackendOutput::event(None))
    }
}

struct QuitAfterNotifications {
    notifications: Rc<Cell<usize>>,
    expected: usize,
}

impl ApplicationEvents for QuitAfterNotifications {
    fn poll_next_event(&mut self, _cx: &mut Context<'_>) -> Poll<intuigram_tui::Result<Event>> {
        if self.notifications.get() >= self.expected {
            Poll::Ready(Ok(key('q')))
        } else {
            Poll::Pending
        }
    }
}

#[test]
fn live_update_burst_does_not_starve_independent_adapter_effects() {
    let notifications = Rc::new(Cell::new(0));
    let emitted = Rc::new(Cell::new(0));
    let mut terminal = RecordingUi {
        views: Rc::new(std::cell::RefCell::new(Vec::new())),
    };
    let mut events = QuitAfterNotifications {
        notifications: Rc::clone(&notifications),
        expected: 1,
    };
    let mut adapter_events = BurstAdapterEvents {
        next_message: 0,
        total: EFFECT_CAPACITY,
        emitted,
    };
    let backend = CountingBackend {
        notifications: Rc::clone(&notifications),
    };
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");

    let result = runtime.block_on(run_application(
        &mut terminal,
        &mut events,
        &mut adapter_events,
        backend,
        intuigram_telegram::PeerDirectory::default(),
        application_fixture(),
    ));

    if let Err(error) = result {
        panic!(
            "a live update burst failed after {} notifications: {error}",
            notifications.get()
        );
    }

    assert!(
        (1..EFFECT_CAPACITY).contains(&notifications.get()),
        "a live burst should make bounded notification progress"
    );
}

#[test]
fn active_notification_replaces_the_same_pending_identity() {
    let notification = Effect::Notify {
        identity: "telegram:7".to_owned(),
        chat: ChatId(101),
    };
    let active_notifications =
        vec![notification_key(&notification).expect("notification has a replacement identity")];
    let active = futures_util::stream::FuturesUnordered::new();
    let mut pending = VecDeque::new();

    enqueue_effect(
        &mut pending,
        &active,
        &active_notifications,
        Some(notification),
    )
    .expect("replacement should not consume effect capacity");

    assert!(pending.is_empty());
}

#[derive(Clone)]
struct StalledHistoryBackend {
    emitted: Rc<Cell<usize>>,
    notifications: Rc<Cell<usize>>,
}

impl ApplicationBackend for StalledHistoryBackend {
    async fn execute(
        &self,
        effect: AdapterEffect,
        _peers: intuigram_telegram::PeerDirectory,
    ) -> Result<BackendOutput> {
        match effect.effect {
            Effect::LoadChat { chat, .. } => {
                poll_fn(|cx| {
                    if self.emitted.get() == EFFECT_CAPACITY {
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
            Effect::Notify { .. } => {
                self.notifications
                    .set(self.notifications.get().saturating_add(1));
                Ok(BackendOutput::event(None))
            }
            _ => Ok(BackendOutput::event(None)),
        }
    }
}

#[test]
fn stalled_history_load_bounds_live_notification_burst() {
    let emitted = Rc::new(Cell::new(0));
    let notifications = Rc::new(Cell::new(0));
    let mut terminal = RecordingUi {
        views: Rc::new(std::cell::RefCell::new(Vec::new())),
    };
    let mut events = QuitAfterNotifications {
        notifications: Rc::clone(&notifications),
        expected: 1,
    };
    let mut adapter_events = BurstAdapterEvents {
        next_message: 0,
        total: EFFECT_CAPACITY,
        emitted: Rc::clone(&emitted),
    };
    let backend = StalledHistoryBackend {
        emitted,
        notifications: Rc::clone(&notifications),
    };
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");

    let result = runtime.block_on(run_application(
        &mut terminal,
        &mut events,
        &mut adapter_events,
        backend,
        intuigram_telegram::PeerDirectory::default(),
        application_fixture(),
    ));

    if let Err(error) = result {
        panic!(
            "a stalled history load rejected live updates after {} emissions: {error}",
            EFFECT_CAPACITY
        );
    }

    assert!(
        (1..EFFECT_CAPACITY).contains(&notifications.get()),
        "a live burst should be coalesced below the effect capacity"
    );
}

#[derive(Clone)]
struct NotificationUnblocksHistoryBackend {
    notifications: Rc<Cell<usize>>,
}

impl ApplicationBackend for NotificationUnblocksHistoryBackend {
    async fn execute(
        &self,
        effect: AdapterEffect,
        _peers: intuigram_telegram::PeerDirectory,
    ) -> Result<BackendOutput> {
        match effect.effect {
            Effect::LoadChat { chat, .. } => {
                poll_fn(|cx| {
                    if self.notifications.get() == 1 {
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
            Effect::Notify { .. } => {
                self.notifications.set(1);
                Ok(BackendOutput::event(None))
            }
            _ => Ok(BackendOutput::event(None)),
        }
    }
}

#[test]
fn stalled_history_does_not_block_independent_notification_work() {
    let notifications = Rc::new(Cell::new(0));
    let mut terminal = RecordingUi {
        views: Rc::new(std::cell::RefCell::new(Vec::new())),
    };
    let mut events = QuitAfterNotifications {
        notifications: Rc::clone(&notifications),
        expected: 1,
    };
    let mut adapter_events = BurstAdapterEvents {
        next_message: 0,
        total: 1,
        emitted: Rc::new(Cell::new(0)),
    };
    let backend = NotificationUnblocksHistoryBackend {
        notifications: Rc::clone(&notifications),
    };
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");

    let result = runtime.block_on(compio::time::timeout(
        Duration::from_millis(250),
        run_application(
            &mut terminal,
            &mut events,
            &mut adapter_events,
            backend,
            intuigram_telegram::PeerDirectory::default(),
            application_fixture(),
        ),
    ));

    assert!(
        matches!(result, Ok(Ok(_))),
        "notification work stayed queued behind the stalled history request"
    );
    assert_eq!(notifications.get(), 1);
}

#[derive(Clone)]
struct ShutdownOrderBackend {
    phase: Rc<Cell<usize>>,
    drained_before_shutdown: Rc<Cell<bool>>,
}

impl ApplicationBackend for ShutdownOrderBackend {
    async fn execute(
        &self,
        effect: AdapterEffect,
        _peers: intuigram_telegram::PeerDirectory,
    ) -> Result<BackendOutput> {
        if matches!(effect.effect, Effect::Notify { .. }) {
            poll_fn(|cx| {
                if self.phase.get() == 0 {
                    self.phase.set(1);
                    cx.waker().wake_by_ref();
                    Poll::Pending
                } else if self.phase.get() == 2 {
                    Poll::Ready(())
                } else {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            })
            .await;
        }
        Ok(BackendOutput::event(None))
    }

    fn begin_shutdown(&self) {
        self.phase.set(2);
    }

    async fn shutdown(self) -> Result<()> {
        self.drained_before_shutdown.set(self.phase.get() == 2);
        Ok(())
    }
}

#[test]
fn quit_drains_accepted_effects_before_backend_shutdown() {
    let phase = Rc::new(Cell::new(0));
    let drained_before_shutdown = Rc::new(Cell::new(false));
    let mut terminal = RecordingUi {
        views: Rc::new(std::cell::RefCell::new(Vec::new())),
    };
    let mut events = QuitAfterNotifications {
        notifications: Rc::clone(&phase),
        expected: 1,
    };
    let mut adapter_events = BurstAdapterEvents {
        next_message: 0,
        total: 1,
        emitted: Rc::new(Cell::new(0)),
    };
    let backend = ShutdownOrderBackend {
        phase,
        drained_before_shutdown: Rc::clone(&drained_before_shutdown),
    };
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
        .expect("application should shut down cleanly");

    assert!(
        drained_before_shutdown.get(),
        "backend shutdown ran before an accepted effect completed"
    );
}

#[derive(Clone)]
struct FatalBackend {
    shutdown: Rc<Cell<bool>>,
}

impl ApplicationBackend for FatalBackend {
    async fn execute(
        &self,
        effect: AdapterEffect,
        _peers: intuigram_telegram::PeerDirectory,
    ) -> Result<BackendOutput> {
        if matches!(effect.effect, Effect::Notify { .. }) {
            return Err(super::super::Error::TelegramActorMailboxClosed);
        }
        Ok(BackendOutput::event(None))
    }

    async fn shutdown(self) -> Result<()> {
        self.shutdown.set(true);
        Ok(())
    }
}

#[test]
fn fatal_backend_error_still_joins_backend_before_returning() {
    let shutdown = Rc::new(Cell::new(false));
    let mut terminal = RecordingUi {
        views: Rc::new(std::cell::RefCell::new(Vec::new())),
    };
    let mut events = QuitAfterNotifications {
        notifications: Rc::new(Cell::new(0)),
        expected: usize::MAX,
    };
    let mut adapter_events = BurstAdapterEvents {
        next_message: 0,
        total: 1,
        emitted: Rc::new(Cell::new(0)),
    };
    let backend = FatalBackend {
        shutdown: Rc::clone(&shutdown),
    };
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");

    let result = runtime.block_on(run_application(
        &mut terminal,
        &mut events,
        &mut adapter_events,
        backend,
        intuigram_telegram::PeerDirectory::default(),
        application_fixture(),
    ));
    let Err(error) = result else {
        panic!("the fatal backend error should be returned after shutdown")
    };

    assert!(matches!(
        error,
        super::super::Error::TelegramActorMailboxClosed
    ));
    assert!(shutdown.get(), "fatal return bypassed backend shutdown");
}
