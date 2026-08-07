//! Hermetic, synchronous behavior-test harness for Intuigram.

mod error;
mod screen;
mod system;
mod telegram;
mod trace;

pub use error::{Artifact, Error, Result};
pub use screen::{
    ActionLocator, ChatLocator, ComposerLocator, MediaCardLocator, MessageLocator, Screen,
};
pub use system::{TelegramControl, TestKey, TestSystem, TestSystemBuilder, key};
pub use telegram::{AccountFixture, TelegramScenario, account, chat, incoming, sent_message};
