use super::Action;

/// A terminal key independent of a concrete terminal backend.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Key {
    /// Printable character.
    Char(char),
    /// Up arrow.
    Up,
    /// Down arrow.
    Down,
    /// Left arrow.
    Left,
    /// Right arrow.
    Right,
    /// Home key.
    Home,
    /// End key.
    End,
    /// Enter key.
    Enter,
    /// Escape key.
    Escape,
}

/// A terminal key with modifier state.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct KeyChord {
    pub(super) key: Key,
    pub(super) control: bool,
    pub(super) shift: bool,
    pub(super) alt: bool,
}

/// One context-sensitive shortcut shown in the Action Bar and Help.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Binding {
    /// Shortcut accepted by input handling.
    pub key: KeyChord,
    /// User-facing action label.
    pub label: &'static str,
    /// Application action produced by the shortcut.
    pub action: Action,
    /// Whether this is the compact Action Bar binding for its action.
    pub primary: bool,
}

impl KeyChord {
    /// Creates an unmodified key.
    #[must_use]
    pub const fn plain(key: Key) -> Self {
        Self {
            key,
            control: false,
            shift: false,
            alt: false,
        }
    }

    /// Creates a Control-modified key.
    #[must_use]
    pub const fn control(key: Key) -> Self {
        Self {
            key,
            control: true,
            shift: false,
            alt: false,
        }
    }

    /// Creates an Alt-modified key.
    #[must_use]
    pub const fn alt(key: Key) -> Self {
        Self {
            key,
            control: false,
            shift: false,
            alt: true,
        }
    }

    /// Creates a Shift-modified key.
    #[must_use]
    pub const fn shift(key: Key) -> Self {
        Self {
            key,
            control: false,
            shift: true,
            alt: false,
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
        label.push_str(match self.key {
            Key::Char(character) => return format!("{label}{}", character.to_ascii_uppercase()),
            Key::Up => "Up",
            Key::Down => "Down",
            Key::Left => "Left",
            Key::Right => "Right",
            Key::Home => "Home",
            Key::End => "End",
            Key::Enter => "Enter",
            Key::Escape => "Esc",
        });
        label
    }
}
use super::*;

impl EffectiveKeymap {
    /// Creates the built-in keymap.
    #[must_use]
    pub const fn defaults() -> Self {
        Self
    }

    /// Resolves a key only when its action is valid in the current view.
    #[must_use]
    pub fn resolve(&self, view: &View, key: KeyChord) -> Option<Action> {
        self.help(view)
            .find(|binding| binding.key == key)
            .map(|binding| binding.action)
    }

    /// Produces compact Action Bar bindings.
    pub fn action_bar<'a>(&'a self, view: &'a View) -> impl Iterator<Item = &'static Binding> + 'a {
        self.help(view).filter(|binding| binding.primary)
    }

    /// Produces exhaustive context Help from the same bindings used for input.
    pub fn help<'a>(&'a self, view: &'a View) -> impl Iterator<Item = &'static Binding> + 'a {
        BINDINGS
            .iter()
            .chain(render::accounts::ACCOUNT_BINDINGS)
            .chain(render::folder_manager::FOLDER_BINDINGS)
            .chain(render::rich_media::RICH_MEDIA_BINDINGS)
            .chain(render::scheduled::SCHEDULED_BINDINGS)
            .filter(|binding| {
                view.actions.contains(&binding.action) && binding_matches_context(view, binding)
            })
    }
}

fn binding_matches_context(view: &View, binding: &Binding) -> bool {
    match (binding.action, binding.key) {
        (Action::OpenActions, key) if key == KeyChord::plain(Key::Char('a')) => {
            view.focus == Focus::Transcript
        }
        (Action::OpenActions, key) if key == KeyChord::alt(Key::Char('a')) => {
            matches!(view.focus, Focus::Chats | Focus::Composer)
        }
        (Action::TargetPreviousMessage, key) if key == KeyChord::plain(Key::Up) => {
            view.focus == Focus::Transcript
        }
        (Action::TargetPreviousMessage, key) if key == KeyChord::alt(Key::Up) => {
            matches!(view.focus, Focus::Composer | Focus::Transcript)
        }
        (Action::TargetNextMessage, key) if key == KeyChord::plain(Key::Down) => {
            view.focus == Focus::Transcript
        }
        (Action::TargetNextMessage, key) if key == KeyChord::alt(Key::Down) => {
            view.focus == Focus::Transcript
        }
        _ => true,
    }
}

impl Default for EffectiveKeymap {
    fn default() -> Self {
        Self::defaults()
    }
}
