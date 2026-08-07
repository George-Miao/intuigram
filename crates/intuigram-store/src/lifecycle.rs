//! Explicit deletion of one Account's durable local records.

use std::fs;
use std::path::PathBuf;

use snafu::{ResultExt, Snafu};

use crate::{AccountId, StoreLayout};

/// Failure while removing explicitly selected Account data.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    /// The Account data directory could not be inspected.
    #[snafu(display("failed to inspect Account data directory {}", path.display()))]
    Inspect {
        path: PathBuf,
        source: std::io::Error,
    },

    /// An exact Account-owned database file could not be removed.
    #[snafu(display("failed to remove Account data file {}", path.display()))]
    Remove {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Result returned by Account-data lifecycle operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Exact local durable-data removal scope for one Account.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AccountDataRemoval {
    /// Account database, SQLite sidecars, and retained recovery backups
    /// removed.
    pub removed: Vec<PathBuf>,
}

impl AccountDataRemoval {
    /// Removes only files whose names are owned by the selected decimal Account
    /// database. The cross-Account registry and Media Cache are separate.
    pub fn clear(layout: &StoreLayout, account: AccountId) -> Result<Self> {
        let directory = layout.data_directory();
        let children = match fs::read_dir(directory) {
            Ok(children) => children,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(source) => {
                return Err(Error::Inspect {
                    path: directory.to_path_buf(),
                    source,
                });
            }
        };
        let database_name = format!("{}.db", account.get());
        let mut removed = Vec::new();
        for child in children {
            let child = child.context(InspectSnafu {
                path: directory.to_path_buf(),
            })?;
            let name = child.file_name();
            let name = name.to_string_lossy();
            let owned = name == database_name
                || name == format!("{database_name}-wal")
                || name == format!("{database_name}-shm")
                || (name.starts_with(&format!("{database_name}.")) && name.ends_with(".bak"));
            if !owned {
                continue;
            }
            let path = child.path();
            fs::remove_file(&path).context(RemoveSnafu { path: path.clone() })?;
            removed.push(path);
        }
        removed.sort();
        Ok(Self { removed })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::AccountDataRemoval;
    use crate::{AccountId, StoreLayout};

    #[test]
    fn clear_is_precise_to_one_account_and_includes_recovery_material() {
        let temporary = tempdir().expect("temporary data root should be created");
        let layout = StoreLayout::new(temporary.path());
        let account = AccountId::new(7).expect("fixture ID should be positive");
        for name in ["7.db", "7.db-wal", "7.db-shm", "7.db.recovery-1.bak"] {
            fs::write(temporary.path().join(name), b"private")
                .expect("Account fixture should be created");
        }
        fs::write(temporary.path().join("8.db"), b"other")
            .expect("other Account fixture should be created");
        fs::write(temporary.path().join("global.db"), b"registry")
            .expect("registry fixture should be created");

        let result = AccountDataRemoval::clear(&layout, account)
            .expect("selected Account data should clear");

        assert_eq!(result.removed.len(), 4);
        assert!(temporary.path().join("8.db").exists());
        assert!(temporary.path().join("global.db").exists());
    }
}
