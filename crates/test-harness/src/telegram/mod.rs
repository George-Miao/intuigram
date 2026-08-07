//! Strict typed Telegram scenarios and deterministic fixtures.

mod command;
mod fixture;
mod scenario;

pub use fixture::{AccountFixture, account, chat, incoming, sent_message};
pub(crate) use scenario::{HistoryResult, ObservedSend};
pub use scenario::{ScenarioMismatch, TelegramScenario};
