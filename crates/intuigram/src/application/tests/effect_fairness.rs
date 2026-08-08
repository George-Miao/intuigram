use std::cell::Cell;
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
}

impl ApplicationAdapterEvents for BurstAdapterEvents {
    fn poll_adapter_event(&mut self, _cx: &mut Context<'_>) -> Poll<Result<AdapterBatch>> {
        if self.next_message >= EFFECT_CAPACITY {
            return Poll::Pending;
        }
        self.next_message += 1;
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
}

impl ApplicationEvents for QuitAfterNotifications {
    fn poll_next_event(&mut self, _cx: &mut Context<'_>) -> Poll<intuigram_tui::Result<Event>> {
        if self.notifications.get() == EFFECT_CAPACITY {
            Poll::Ready(Ok(key('q')))
        } else {
            Poll::Pending
        }
    }
}

#[test]
fn live_update_burst_does_not_starve_pending_adapter_effects() {
    let notifications = Rc::new(Cell::new(0));
    let mut terminal = RecordingUi {
        views: Rc::new(std::cell::RefCell::new(Vec::new())),
    };
    let mut events = QuitAfterNotifications {
        notifications: Rc::clone(&notifications),
    };
    let mut adapter_events = BurstAdapterEvents { next_message: 0 };
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
