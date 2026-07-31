//! Terminal interaction policy shared by input handling and rendering.

use popgram_app::{Action, View};

/// A terminal key with its modifier state.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct KeyChord {
    /// Printable key, normalized to lowercase.
    key: char,
    /// Whether Control is held.
    control: bool,
    /// Whether Shift is held.
    shift: bool,
    /// Whether Alt is held.
    alt: bool,
}

impl KeyChord {
    /// Creates a Control-modified printable key.
    #[must_use]
    pub const fn control(key: char) -> Self {
        Self {
            key: key.to_ascii_lowercase(),
            control: true,
            shift: false,
            alt: false,
        }
    }

    /// Creates an Alt-modified printable key.
    #[must_use]
    pub const fn alt(key: char) -> Self {
        Self {
            key: key.to_ascii_lowercase(),
            control: false,
            shift: false,
            alt: true,
        }
    }

    /// Formats the chord exactly as the terminal UI displays it.
    #[must_use]
    pub fn label(self) -> String {
        let mut label = String::new();
        if self.control {
            label.push_str("Ctrl+");
        }
        if self.alt {
            label.push_str("Alt+");
        }
        if self.shift {
            label.push_str("Shift+");
        }
        label.push(self.key.to_ascii_uppercase());
        label
    }
}

/// One context-sensitive shortcut shown in the Action Bar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionHint {
    /// Shortcut accepted by input handling.
    pub key: KeyChord,
    /// User-facing action label.
    pub label: &'static str,
    /// Application action produced by the shortcut.
    pub action: Action,
}

/// Effective bindings for the active configuration.
pub struct EffectiveKeymap;

const RECONNECT: ActionHint = ActionHint {
    key: KeyChord::alt('r'),
    label: "Reconnect",
    action: Action::Reconnect,
};

impl EffectiveKeymap {
    /// Creates the built-in keymap.
    #[must_use]
    pub const fn defaults() -> Self {
        Self
    }

    /// Resolves a key only when its action is valid in the current view.
    #[must_use]
    pub fn resolve(&self, view: &View, key: KeyChord) -> Option<Action> {
        self.action_bar(view)
            .into_iter()
            .find(|hint| hint.key == key)
            .map(|hint| hint.action)
    }

    /// Produces Action Bar hints from the same bindings used for input.
    #[must_use]
    pub fn action_bar(&self, view: &View) -> Vec<ActionHint> {
        [RECONNECT]
            .into_iter()
            .filter(|hint| view.actions.contains(&hint.action))
            .collect()
    }
}

impl Default for EffectiveKeymap {
    fn default() -> Self {
        Self::defaults()
    }
}

#[cfg(test)]
mod tests {
    use popgram_app::{Action, ConnectionState, View};

    use super::{EffectiveKeymap, KeyChord};

    #[test]
    fn displayed_action_bar_binding_is_the_binding_input_resolves() {
        let view = View {
            connection: ConnectionState::ReconnectCooldown,
            actions: vec![Action::Reconnect],
        };
        let keymap = EffectiveKeymap::defaults();

        let hints = keymap.action_bar(&view);
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].key.label(), "Alt+R");
        assert_eq!(keymap.resolve(&view, hints[0].key), Some(Action::Reconnect));

        let connected = View {
            connection: ConnectionState::Connected,
            actions: Vec::new(),
        };
        assert_eq!(keymap.resolve(&connected, KeyChord::alt('r')), None);
        assert!(keymap.action_bar(&connected).is_empty());
    }
}
