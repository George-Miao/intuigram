//! Durable, account-isolated storage for Intuigram.

mod account;
mod global;
mod layout;
mod lifecycle;
mod recovery;

pub use account::{
    AccountCipher, AccountDatabase, AccountStore, CachedAccount, DatabaseRequest, Error, Result,
    SecurityError, SecurityResult, SessionMaterial, StoredChat, StoredDraft, StoredFolder,
    StoredMessage, StoredMutation, StoredSelection, StoredTranscriptAnchor, SyncBatch, SyncCursor,
    enable_local_lock, local_lock_is_enabled,
};
pub use global::{AccountRecord, Error as GlobalError, GlobalDatabase, Result as GlobalResult};
pub use layout::{AccountId, StoreLayout};
pub use lifecycle::{AccountDataRemoval, Error as LifecycleError, Result as LifecycleResult};
pub use recovery::{AccountOpen, AccountRecovery, RebuiltAccount, RecoveryError, RecoveryResult};
