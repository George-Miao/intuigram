//! Durable, account-isolated storage for Popgram.

mod account;
mod layout;

pub use account::{AccountDatabase, Error, Result};
pub use layout::{AccountId, StoreLayout};
