use std::path::{Path, PathBuf};

/// A positive Telegram user identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AccountId(i64);

impl AccountId {
    /// Creates an account identifier when `value` is positive.
    #[must_use]
    pub const fn new(value: i64) -> Option<Self> {
        if value > 0 { Some(Self(value)) } else { None }
    }

    /// Returns the raw Telegram user identifier.
    #[must_use]
    pub const fn get(self) -> i64 {
        self.0
    }
}

/// Resolves Popgram's durable database paths.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreLayout {
    data: PathBuf,
}

impl StoreLayout {
    /// Creates a layout rooted at the platform-specific Popgram data directory.
    #[must_use]
    pub fn new(data: impl Into<PathBuf>) -> Self {
        Self { data: data.into() }
    }

    /// Returns the durable data directory.
    #[must_use]
    pub fn data_directory(&self) -> &Path {
        &self.data
    }

    /// Returns the cross-account database path.
    #[must_use]
    pub fn global_database(&self) -> PathBuf {
        self.data.join("global.db")
    }

    /// Returns the incomplete-login database path.
    #[must_use]
    pub fn pending_database(&self) -> PathBuf {
        self.data.join(".pending.db")
    }

    /// Returns an account database path.
    #[must_use]
    pub fn account_database(&self, account: AccountId) -> PathBuf {
        self.data.join(format!("{}.db", account.get()))
    }
}
