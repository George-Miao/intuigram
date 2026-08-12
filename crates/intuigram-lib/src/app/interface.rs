use super::*;

impl App {
    /// Creates an application waiting for initial adapter data.
    #[must_use]
    pub fn new() -> Self {
        let mut app = Self::empty();
        app.refresh_actions();
        app
    }

    /// Applies one ordered input and returns the resulting immutable view and
    /// adapter effect.
    #[must_use]
    pub fn transition(&mut self, input: Input) -> Update {
        let effect = self.apply(input);
        self.sync_avatar_load_view();
        self.refresh_actions();
        Update {
            view: self.view.clone(),
            effect,
        }
    }

    /// Returns the current immutable view without changing application state.
    #[must_use]
    pub fn view(&self) -> View {
        self.view.clone()
    }
}
