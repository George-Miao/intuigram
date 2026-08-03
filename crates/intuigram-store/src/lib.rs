//! Durable, account-isolated storage for Intuigram.

mod account;
mod global;
mod layout;

pub use account::{
    AccountDatabase, AccountStore, CachedAccount, DatabaseRequest, Error, Result, SessionMaterial,
    StoredChat, StoredDraft, StoredFolder, StoredMessage, SyncBatch, SyncCursor,
};
pub use global::{AccountRecord, Error as GlobalError, GlobalDatabase, Result as GlobalResult};
pub use layout::{AccountId, StoreLayout};
