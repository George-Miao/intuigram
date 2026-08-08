use std::cell::Cell;
use std::future::poll_fn;
use std::rc::Rc;
use std::task::{Context, Poll};

use crossterm::event::Event;
use intuigram_app::{
    AdapterEvent, ChatId, DeliveryState, Effect, MessageDetails, MessageDirection, MessageId,
    MessageView,
};

use super::super::runtime_adapters::{AdapterBatch, BackendOutput};
use super::super::{
    AdapterEffect, ApplicationAdapterEvents, ApplicationBackend, ApplicationEvents,
    EFFECT_CAPACITY, Result, application_fixture, run_application,
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

struct CountingBackend {
    notifications: Rc<Cell<usize>>,
}

impl ApplicationBackend for CountingBackend {
    async fn execute(
        &mut self,
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
        if self.notifications.get() == self.expected {
            Poll::Ready(Ok(key('q')))
        } else {
            Poll::Pending
        }
    }
}

#[test]
fn live_update_burst_does_not_starve_pending_adapter_effects() {
    let notifications = Rc::new(Cell::new(0));
    let emitted = Rc::new(Cell::new(0));
    let mut terminal = RecordingUi {
        views: Rc::new(std::cell::RefCell::new(Vec::new())),
    };
    let mut events = QuitAfterNotifications {
        notifications: Rc::clone(&notifications),
        expected: EFFECT_CAPACITY,
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

    assert_eq!(notifications.get(), EFFECT_CAPACITY);
}

struct StalledHistoryBackend {
    emitted: Rc<Cell<usize>>,
    notifications: Rc<Cell<usize>>,
}

impl ApplicationBackend for StalledHistoryBackend {
    async fn execute(
        &mut self,
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
fn stalled_history_load_coalesces_live_notification_burst() {
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

    assert_eq!(notifications.get(), 1);
}
