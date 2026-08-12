//! Synchronous state driver for hermetic behavior tests.

use std::collections::VecDeque;

use intuigram_lib::{AdapterEvent, App, Bootstrap, Effect, Input, View};
use intuigram_tui::UiEvent;

/// Synchronous application-state driver used by hermetic behavior tests.
///
/// It owns the reducer, translates terminal events into reducer inputs, and
/// retains requested adapter effects until the composition layer accepts them.
pub(super) struct Driver {
    app: App,
    view: View,
    effects: VecDeque<Effect>,
    revision: u64,
}

impl Driver {
    /// Creates a state owner from one normalized adapter bootstrap.
    #[must_use]
    pub(super) fn new(bootstrap: Bootstrap) -> Self {
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
    pub(super) const fn view(&self) -> &View {
        &self.view
    }

    /// Returns the monotonically increasing state revision.
    #[must_use]
    pub(super) const fn revision(&self) -> u64 {
        self.revision
    }

    /// Applies one event resolved by the production terminal keymap.
    pub(super) fn handle_ui(&mut self, event: UiEvent) {
        match event {
            UiEvent::Intent(intent) => self.transition(Input::Intent(intent)),
            UiEvent::Redraw => {}
        }
    }

    /// Applies one normalized adapter event.
    pub(super) fn handle_adapter(&mut self, event: AdapterEvent) {
        self.transition(Input::Adapter(event));
    }

    /// Takes the oldest adapter effect requested by state transitions.
    pub(super) fn take_effect(&mut self) -> Option<Effect> {
        let effect = self.effects.pop_front()?;
        if let Some(admission) = effect.admission() {
            self.transition(Input::EffectAccepted(admission));
        }
        Some(effect)
    }

    /// Reports whether an adapter effect is waiting to be accepted.
    #[must_use]
    pub(super) fn has_pending_effects(&self) -> bool {
        !self.effects.is_empty()
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
