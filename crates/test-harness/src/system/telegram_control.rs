//! Telegram-side controls for a running hermetic application.

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread;

use intuigram_app::{AdapterEvent, MessageView};
use intuigram_telegram::{LiveEvent, UpdateCursor};
use snafu::ResultExt;

use super::TestSystem;
use crate::error::{Error, Result, SyncSnafu};
use crate::telegram::{AccountFixture, ScenarioMismatch};

/// Explicit Telegram events and completions available to behavior scenarios.
pub struct TelegramControl<'a> {
    pub(super) system: &'a mut TestSystem,
}

impl TelegramControl<'_> {
    /// Restores a cached Account snapshot after a connection handoff.
    pub fn restore(&mut self, account: AccountFixture) -> Result<()> {
        self.inject(AdapterEvent::ConnectionRestored(account.into_bootstrap()))
    }

    /// Commits a cursor-bearing live update before exposing it to the
    /// application.
    pub fn inject_update(&mut self, cursor: UpdateCursor, event: AdapterEvent) -> Result<()> {
        self.system.trace.borrow_mut().record(
            "telegram-update",
            format!("cursor {cursor:?}: {event:?}"),
            self.system.application.revision(),
        );
        let commit = self
            .system
            .updates
            .commit(LiveEvent {
                events: vec![event],
                cursors: vec![cursor],
                peers: intuigram_telegram::PeerDirectory::default(),
            })
            .context(SyncSnafu)?;
        let update = block_on(commit).context(SyncSnafu)?;
        for event in update.events {
            self.system.application.handle_adapter(event);
        }
        self.system.render();
        self.system.drain_effects()
    }

    /// Injects a connection failure without external timing.
    pub fn disconnect(&mut self) {
        self.system.trace.borrow_mut().record(
            "telegram-event",
            "disconnect",
            self.system.application.revision(),
        );
        self.system
            .application
            .handle_adapter(AdapterEvent::ConnectionFailed(
                "fixture disconnect".to_owned(),
            ));
        self.system.render();
    }

    /// Completes a previously held send with Telegram's acknowledged Message.
    pub fn complete(&mut self, label: &str, mut message: MessageView) -> Result<()> {
        let Some(held) = self.system.telegram.take_held(label) else {
            return Err(Error::TelegramMismatch {
                expected: format!("held send {label:?}"),
                observed: "completion without a matching held send".to_owned(),
                artifact: self.system.trace.borrow().persist(),
            });
        };
        message.body.clone_from(&held.text);
        message.reply_to = held.reply_to;
        message.details.thread_root = held.thread_root;
        self.system.trace.borrow_mut().record(
            "telegram-event",
            format!(
                "complete {label:?}: local {} -> {}",
                held.local_id.0, message.id.0
            ),
            self.system.application.revision(),
        );
        self.system
            .application
            .handle_adapter(AdapterEvent::MessageAdded {
                chat: held.chat,
                message: Box::new(message),
            });
        self.system.render();
        self.system.drain_effects()
    }

    /// Injects a normalized adapter event that has no synchronization cursor.
    pub fn inject(&mut self, event: AdapterEvent) -> Result<()> {
        self.system.trace.borrow_mut().record(
            "telegram-event",
            format!("{event:?}"),
            self.system.application.revision(),
        );
        self.system.application.handle_adapter(event);
        self.system.render();
        self.system.drain_effects()
    }
}

impl TestSystem {
    pub(super) fn scenario_error(&self, error: ScenarioMismatch) -> Error {
        Error::TelegramMismatch {
            expected: error.expected,
            observed: error.observed,
            artifact: self.trace.borrow().persist(),
        }
    }
}

struct ThreadWaker(thread::Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

pub(super) fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => thread::park(),
        }
    }
}
