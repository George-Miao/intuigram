use std::path::{Path, PathBuf};

use super::error::{RecoveryError, RecoveryResult};
use super::rebuild::rebuild;
use super::snapshot::{discover_backups, read_unique_records};
use crate::{
    AccountCipher, AccountDatabase, AccountId, SessionMaterial, StoreLayout, StoredDraft, account,
};

/// Result of opening an existing Account without silently changing its data.
pub enum AccountOpen {
    /// The database passed migrations and consistency checks.
    Ready(AccountDatabase),

    /// The database remains untouched and requires an explicit recovery choice.
    Recovery(Box<AccountRecovery>),
}

/// A failed Account open plus the evidence needed for safe recovery choices.
pub struct AccountRecovery {
    layout: StoreLayout,
    account: AccountId,
    cipher: AccountCipher,
    database_path: PathBuf,
    backup_paths: Vec<PathBuf>,
    cause: Box<account::Error>,
    snapshot: RecoveryResult<UniqueRecords>,
}

impl AccountRecovery {
    pub(crate) fn inspect(
        layout: &StoreLayout,
        account: AccountId,
        cipher: AccountCipher,
        cause: account::Error,
    ) -> Self {
        let database_path = layout.account_database(account);
        Self {
            backup_paths: discover_backups(&database_path),
            snapshot: read_unique_records(&database_path, account, &cipher),
            layout: layout.clone(),
            account,
            cipher,
            database_path,
            cause: Box::new(cause),
        }
    }

    /// Exact Account database path that failed to open.
    #[must_use]
    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    /// Existing pre-migration and recovery backups, newest name last.
    #[must_use]
    pub fn backup_paths(&self) -> &[PathBuf] {
        &self.backup_paths
    }

    /// Original typed storage failure that entered recovery.
    #[must_use]
    pub const fn cause(&self) -> &account::Error {
        &self.cause
    }

    /// Whether every unique record in the current schema was proven readable.
    #[must_use]
    pub const fn can_rebuild_cache(&self) -> bool {
        self.snapshot.is_ok()
    }

    /// Why a safe cache rebuild cannot currently be offered.
    #[must_use]
    pub fn rebuild_blocker(&self) -> Option<&RecoveryError> {
        self.snapshot.as_ref().err()
    }

    /// Retries the normal migration and consistency-check path without
    /// modifying the database on failure.
    pub fn retry(self) -> account::Result<AccountOpen> {
        AccountDatabase::open_recoverable_with_cipher(&self.layout, self.account, self.cipher)
    }

    /// Replaces only redownloadable synchronized tables after copying all
    /// verified unique records into a fresh schema.
    pub fn rebuild_cache(self) -> RecoveryResult<RebuiltAccount> {
        let records = self.snapshot?;
        rebuild(
            self.layout,
            self.account,
            self.cipher,
            self.database_path,
            records,
        )
    }
}

/// A safely rebuilt Account and the exact path retaining its original bytes.
pub struct RebuiltAccount {
    pub(super) database: AccountDatabase,
    pub(super) preserved_original: PathBuf,
}

impl RebuiltAccount {
    /// Reopened Account database containing the preserved unique records.
    #[must_use]
    pub const fn database(&self) -> &AccountDatabase {
        &self.database
    }

    /// Original failed database, retained for export or forensic recovery.
    #[must_use]
    pub fn preserved_original(&self) -> &Path {
        &self.preserved_original
    }

    /// Consumes the outcome and returns the usable database.
    #[must_use]
    pub fn into_database(self) -> AccountDatabase {
        self.database
    }
}

pub(super) struct UniqueRecords {
    pub(super) account: AccountId,
    pub(super) session: Option<SessionMaterial>,
    pub(super) drafts: Vec<StoredDraft>,
    pub(super) draft_history: Vec<DraftHistory>,
}

pub(super) struct DraftHistory {
    pub(super) chat_id: i64,
    pub(super) thread_root: Option<i64>,
    pub(super) saved_peer: Option<i64>,
    pub(super) text: String,
    pub(super) reply_to: Option<i64>,
    pub(super) displaced_at: i64,
}
