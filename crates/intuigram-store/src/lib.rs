//! Durable, account-isolated storage for Intuigram.

mod account;
mod global;
mod layout;

pub use account::{AccountDatabase, Error, Result, SessionMaterial};
pub use global::{AccountRecord, Error as GlobalError, GlobalDatabase, Result as GlobalResult};
pub use layout::{AccountId, StoreLayout};
