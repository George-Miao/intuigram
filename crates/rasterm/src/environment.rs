use std::ffi::{OsStr, OsString};
use std::path::Path;

use crate::Protocol;

/// Multiplexer between the application and the outer terminal.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Multiplexer {
    /// Direct terminal session.
    #[default]
    None,

    /// tmux DCS passthrough is required for graphics commands.
    Tmux,

    /// Zellij handles modern Kitty/Sixel graphics natively.
    Zellij,
}

/// Terminal facts used for deterministic protocol selection.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Environment {
    /// `$TERM`.
    pub term: Option<OsString>,

    /// `$TERM_PROGRAM`, retained by several multiplexers.
    pub term_program: Option<OsString>,

    /// Whether Kitty/Ghostty identifies a Kitty-compatible window.
    pub kitty_window: bool,

    /// Active multiplexer.
    pub multiplexer: Multiplexer,

    /// X11 or Wayland is available for an overlay renderer.
    pub graphical_display: bool,

    /// `ueberzugpp` is available on `PATH`.
    pub ueberzugpp: bool,

    /// `chafa` is available on `PATH`.
    pub chafa: bool,
}

impl Environment {
    /// Reads terminal facts without launching a helper process.
    #[must_use]
    pub fn from_env() -> Self {
        let multiplexer = if std::env::var_os("ZELLIJ").is_some() {
            Multiplexer::Zellij
        } else if std::env::var_os("TMUX").is_some() {
            Multiplexer::Tmux
        } else {
            Multiplexer::None
        };
        Self {
            term: std::env::var_os("TERM"),
            term_program: std::env::var_os("TERM_PROGRAM"),
            kitty_window: std::env::var_os("KITTY_WINDOW_ID").is_some(),
            multiplexer,
            graphical_display: std::env::var_os("DISPLAY").is_some()
                || std::env::var_os("WAYLAND_DISPLAY").is_some(),
            ueberzugpp: executable_on_path("ueberzugpp"),
            chafa: executable_on_path("chafa"),
        }
    }

    /// Selects the strongest supported presentation for these facts.
    #[must_use]
    pub fn protocol(&self) -> Protocol {
        let term = text(self.term.as_deref());
        let program = text(self.term_program.as_deref());
        if self.multiplexer == Multiplexer::Zellij {
            return Protocol::Sixel;
        }
        if self.kitty_window
            || contains_any(term, &["ghostty", "kitty"])
            || contains_any(program, &["ghostty", "kitty"])
        {
            return Protocol::KittyUnicode;
        }
        if contains_any(term, &["konsole"]) || contains_any(program, &["konsole"]) {
            return Protocol::KittyLegacy;
        }
        if contains_any(program, &["iterm", "wezterm", "warp", "vscode", "tabby"]) {
            return Protocol::Iterm2;
        }
        if contains_any(term, &["sixel", "foot", "mlterm"])
            || contains_any(program, &["windows terminal", "black box"])
        {
            return Protocol::Sixel;
        }
        if self.graphical_display && self.ueberzugpp {
            return Protocol::Ueberzug;
        }
        if self.chafa {
            return Protocol::Chafa;
        }
        Protocol::Text
    }
}

fn executable_on_path(name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|directory| Path::new(&directory).join(name).is_file())
    })
}

fn text(value: Option<&OsStr>) -> &str {
    value.and_then(OsStr::to_str).unwrap_or_default()
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    let value = value.to_ascii_lowercase();
    needles.iter().any(|needle| value.contains(needle))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{Environment, Multiplexer};
    use crate::Protocol;

    #[test]
    fn zellij_uses_its_only_supported_graphics_protocol() {
        let environment = Environment {
            term: Some(OsString::from("xterm-256color")),
            term_program: Some(OsString::from("ghostty")),
            multiplexer: Multiplexer::Zellij,
            ..Environment::default()
        };
        assert_eq!(environment.protocol(), Protocol::Sixel);
    }

    #[test]
    fn alacritty_uses_an_available_external_fallback() {
        let environment = Environment {
            term: Some(OsString::from("alacritty")),
            graphical_display: true,
            ueberzugpp: true,
            ..Environment::default()
        };
        assert_eq!(environment.protocol(), Protocol::Ueberzug);
    }
}
