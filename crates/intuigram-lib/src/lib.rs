//! Deterministic, single-owner application state for Intuigram.

mod app;
mod domain;
mod history;
mod protocol;

pub use app::App;
pub use domain::*;
pub use protocol::*;

#[cfg(test)]
mod tests;
