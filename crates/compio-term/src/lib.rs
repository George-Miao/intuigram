//! Compio-native readiness for Crossterm terminal events.
//!
//! The adapter remains intentionally small: Crossterm owns terminal byte
//! parsing and public event types, while `compio-term` supplies wake-driven
//! readiness that can live on a Compio runtime without a helper thread.

mod event;
mod sys;

pub use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
pub use event::{EventStream, Result};
