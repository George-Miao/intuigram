//! Ratatui terminal adapter and shared effective keymap.

mod recovery;
mod source;

pub use recovery::{RecoveryAction, RecoveryView, resolve_recovery_event};
#[cfg(test)]
pub(crate) use source::qr::render::{chord_from_crossterm, qr_login_symbols};
#[cfg(test)]
pub(crate) use source::render::layout::render;
#[cfg(test)]
pub(crate) use source::terminal::{resolve_event, terminal_keyboard_flags};
pub use source::*;

#[cfg(test)]
mod tests;
