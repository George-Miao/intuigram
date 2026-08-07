//! Durable, account-isolated storage for Intuigram.

mod account;
mod global;
mod layout;
mod recovery;

pub use account::{
    AccountDatabase, AccountStore, CachedAccount, DatabaseRequest, Error, Result, SessionMaterial,
    StoredChat, StoredDraft, StoredFolder, StoredMessage, StoredMutation, StoredSelection,
    SyncBatch, SyncCursor,
};
pub use global::{AccountRecord, Error as GlobalError, GlobalDatabase, Result as GlobalResult};
pub use layout::{AccountId, StoreLayout};
pub use recovery::{AccountOpen, AccountRecovery, RebuiltAccount, RecoveryError, RecoveryResult};
