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
