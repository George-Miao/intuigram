//! Intuigram executable application orchestration.

use std::collections::VecDeque;

use intuigram_lib::{AdapterEvent, App, Bootstrap, Effect, Input, Update, View};
use intuigram_tui::UiEvent;

mod application;
mod operation_providers;
mod recovery;
mod sync;

pub use operation_providers::{
    Clock, Error as ProviderError, OperationIdSource, OperationProviders, OperationStamp,
    Result as ProviderResult, SecureOperationIds, SystemClock,
};
pub use sync::{
    CommittedUpdate, Error as SyncError, Result as SyncResult, UpdateCommit, UpdateCommitter,
    bootstrap_sync_batch, decode_stored_message, encode_stored_message, store_cursor,
};

/// Runs the Intuigram process and reports terminal failures before exiting.
pub fn main() {
    application::main();
}

/// Synchronous application-state driver used by production orchestration and
/// hermetic behavior tests.
///
/// It owns the reducer, translates terminal events into reducer inputs, and
/// retains every requested adapter effect until the composition layer accepts
/// it. External I/O remains the responsibility of adapters.
pub struct Application {
    app: App,
    view: View,
    effects: VecDeque<Effect>,
    revision: u64,
}

impl Application {
    /// Creates a state owner from one normalized adapter bootstrap.
    #[must_use]
    pub fn new(bootstrap: Bootstrap) -> Self {
        let mut app = App::new();
        let update = app.transition(Input::Adapter(AdapterEvent::Bootstrap(bootstrap)));
        let mut application = Self {
            app,
            view: update.view,
            effects: VecDeque::new(),
            revision: 1,
        };
        application.enqueue(update.effect);
        application
    }

    /// Returns the latest immutable render data.
    #[must_use]
    pub const fn view(&self) -> &View {
        &self.view
    }

    /// Returns the monotonically increasing state revision.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Applies one event resolved by the production terminal keymap.
    pub fn handle_ui(&mut self, event: UiEvent) {
        match event {
            UiEvent::Intent(intent) => self.transition(Input::Intent(intent)),
            UiEvent::Redraw => {}
        }
    }

    /// Applies one normalized adapter event.
    pub fn handle_adapter(&mut self, event: AdapterEvent) {
        self.transition(Input::Adapter(event));
    }

    /// Takes the oldest adapter effect requested by state transitions.
    pub fn take_effect(&mut self) -> Option<Effect> {
        self.effects.pop_front()
    }

    /// Reports whether an adapter effect is waiting to be accepted.
    #[must_use]
    pub fn has_pending_effects(&self) -> bool {
        !self.effects.is_empty()
    }

    /// Converts the driver into the lower-level state used by the current
    /// asynchronous executable loop.
    #[must_use]
    pub fn into_parts(mut self) -> (App, Update) {
        let effect = self.effects.pop_front();
        debug_assert!(
            self.effects.is_empty(),
            "initial application bootstrap emits at most one effect"
        );
        (
            self.app,
            Update {
                view: self.view,
                effect,
            },
        )
    }

    fn transition(&mut self, input: Input) {
        let update = self.app.transition(input);
        self.view = update.view;
        self.revision = self.revision.saturating_add(1);
        self.enqueue(update.effect);
    }

    fn enqueue(&mut self, effect: Option<Effect>) {
        if let Some(effect) = effect {
            self.effects.push_back(effect);
        }
    }
}
