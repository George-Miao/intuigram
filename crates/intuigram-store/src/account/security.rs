//! Redacted SQLCipher key material for one unlocked Account.

use std::path::{Path, PathBuf};
use std::{fmt, fs, io};

use rusqlite::Connection;
use snafu::{ResultExt, Snafu};
use zeroize::{ZeroizeOnDrop, Zeroizing};

use crate::{AccountId, StoreLayout};

/// Failure while converting existing Account files to Local Lock.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    /// Local Lock requires encrypted key material.
    #[snafu(display("Local Lock cannot be enabled without an encryption key"))]
    MissingKey,

    /// A stale migration workspace must be inspected rather than overwritten.
    #[snafu(display("Local Lock migration workspace already exists at {}", path.display()))]
    WorkspaceExists { path: PathBuf },

    /// SQLCipher could not export or validate an Account file.
    #[snafu(display("failed to encrypt Account file {}", path.display()))]
    Encrypt {
        path: PathBuf,
        source: rusqlite::Error,
    },

    /// An encrypted file could not atomically replace its plaintext source.
    #[snafu(display("failed to install encrypted Account file {}", path.display()))]
    Install { path: PathBuf, source: io::Error },
}

/// Result returned by Local Lock storage migration.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Optional encryption applied before any Account database access.
#[derive(Clone, Default, Eq, PartialEq, ZeroizeOnDrop)]
pub struct AccountCipher(Option<[u8; 32]>);

impl AccountCipher {
    /// Leaves an Account database in the compatible unencrypted format.
    #[must_use]
    pub const fn plaintext() -> Self {
        Self(None)
    }

    /// Uses a caller-derived raw SQLCipher key.
    #[must_use]
    pub const fn encrypted(key: [u8; 32]) -> Self {
        Self(Some(key))
    }

    pub(crate) fn key_pragma(&self) -> Option<Zeroizing<String>> {
        self.key_literal().map(|literal| {
            let mut pragma = Zeroizing::new(String::with_capacity(literal.len() + 14));
            pragma.push_str("PRAGMA key = ");
            pragma.push_str(&literal);
            pragma.push(';');
            pragma
        })
    }

    fn key_literal(&self) -> Option<Zeroizing<String>> {
        self.0.as_ref().map(|key| {
            let mut literal = Zeroizing::new(String::with_capacity(68));
            literal.push_str("\"x'");
            for byte in key {
                use std::fmt::Write;

                write!(&mut *literal, "{byte:02x}").expect("writing to a String cannot fail");
            }
            literal.push_str("'\"");
            literal
        })
    }

    /// Reports whether the Account database is encrypted.
    #[must_use]
    pub const fn is_encrypted(&self) -> bool {
        self.0.is_some()
    }
}

/// Encrypts the current Account database and every retained Account backup.
pub fn enable_local_lock(
    layout: &StoreLayout,
    account: AccountId,
    cipher: &AccountCipher,
) -> Result<()> {
    let key = cipher.key_literal().ok_or(Error::MissingKey)?;
    let database = layout.account_database(account);
    let directory = layout.data_directory();
    let prefix = format!("{}.db", account.get());
    let children = fs::read_dir(directory).map_err(|source| Error::Install {
        path: directory.to_path_buf(),
        source,
    })?;
    let mut paths = Vec::new();
    for child in children {
        let path = child
            .map_err(|source| Error::Install {
                path: directory.to_path_buf(),
                source,
            })?
            .path();
        if path.file_name().is_some_and(|name| {
            let name = name.to_string_lossy();
            name == prefix || (name.starts_with(&format!("{prefix}.")) && name.ends_with(".bak"))
        }) {
            paths.push(path);
        }
    }
    paths.sort_by_key(|path| (*path != database, path.clone()));
    for path in paths {
        encrypt_file(&path, &key, cipher)?;
    }
    for suffix in ["-wal", "-shm"] {
        let sidecar = directory.join(format!("{prefix}{suffix}"));
        match fs::remove_file(&sidecar) {
            Ok(()) => {}
            Err(source) if source.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(Error::Install {
                    path: sidecar,
                    source,
                });
            }
        }
    }
    Ok(())
}

/// Reports whether the Account's primary database is already encrypted.
#[must_use]
pub fn local_lock_is_enabled(layout: &StoreLayout, account: AccountId) -> bool {
    let path = layout.account_database(account);
    path.is_file() && !is_plaintext(&path)
}

fn encrypt_file(path: &Path, key: &str, cipher: &AccountCipher) -> Result<()> {
    let encrypted = path.with_extension("local-lock-encrypted.tmp");
    let plaintext = path.with_extension("local-lock-plaintext.tmp");
    if plaintext.exists() {
        if !path.exists() && encrypted.exists() {
            fs::rename(&encrypted, path).context(InstallSnafu {
                path: path.to_path_buf(),
            })?;
        }
        if path.exists() && !is_plaintext(path) && !encrypted.exists() {
            protect(path)?;
            return fs::remove_file(&plaintext).context(InstallSnafu { path: plaintext });
        }
        return WorkspaceExistsSnafu { path: plaintext }.fail();
    }
    if encrypted.exists() {
        return WorkspaceExistsSnafu { path: encrypted }.fail();
    }
    if !is_plaintext(path) {
        return Ok(());
    }
    let connection = Connection::open(path).context(EncryptSnafu {
        path: path.to_path_buf(),
    })?;
    let escaped = encrypted.to_string_lossy().replace('\'', "''");
    let mut export = Zeroizing::new(String::with_capacity(escaped.len() + key.len() + 112));
    export.push_str("ATTACH DATABASE '");
    export.push_str(&escaped);
    export.push_str("' AS locked KEY ");
    export.push_str(key);
    export.push_str("; SELECT sqlcipher_export('locked'); DETACH DATABASE locked;");
    connection.execute_batch(&export).context(EncryptSnafu {
        path: path.to_path_buf(),
    })?;
    drop(connection);
    let check = Connection::open(&encrypted).context(EncryptSnafu {
        path: encrypted.clone(),
    })?;
    check
        .execute_batch(
            &cipher
                .key_pragma()
                .expect("Local Lock migration verified an encrypted key"),
        )
        .context(EncryptSnafu {
            path: encrypted.clone(),
        })?;
    check
        .query_row("SELECT count(*) FROM sqlite_master", [], |row| {
            row.get::<_, i64>(0)
        })
        .context(EncryptSnafu {
            path: encrypted.clone(),
        })?;
    drop(check);
    fs::rename(path, &plaintext).context(InstallSnafu {
        path: path.to_path_buf(),
    })?;
    if let Err(source) = fs::rename(&encrypted, path) {
        let _ = fs::rename(&plaintext, path);
        return Err(Error::Install {
            path: path.to_path_buf(),
            source,
        });
    }
    protect(path)?;
    fs::remove_file(&plaintext).context(InstallSnafu { path: plaintext })
}

#[cfg(unix)]
fn protect(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).context(InstallSnafu {
        path: path.to_path_buf(),
    })
}

#[cfg(not(unix))]
fn protect(_path: &Path) -> Result<()> {
    Ok(())
}

fn is_plaintext(path: &Path) -> bool {
    use std::io::Read;

    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let mut header = [0_u8; 16];
    file.read_exact(&mut header).is_ok() && &header == b"SQLite format 3\0"
}

impl fmt::Debug for AccountCipher {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("AccountCipher")
            .field(&if self.is_encrypted() {
                "[REDACTED]"
            } else {
                "plaintext"
            })
            .finish()
    }
}
