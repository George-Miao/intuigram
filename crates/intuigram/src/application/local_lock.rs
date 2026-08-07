//! First-open Local Lock acquisition without exposing secrets to app state.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use intuigram_config::{Config, UnlockMethod};
use intuigram_store::{AccountCipher, AccountId};
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use snafu::{ResultExt, Snafu};
use zeroize::Zeroizing;

const KEYRING_SERVICE: &str = "dev.intuigram.local-lock";
const PASSPHRASE_ROUNDS: u32 = 600_000;
const SALT_BYTES: usize = 32;
const LEGACY_PASSPHRASE_SALT: &[u8] = b"Intuigram Local Lock v1";

/// Failure while acquiring or promoting Local Lock material.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(super)))]
pub(super) enum Error {
    /// A hidden passphrase could not be read.
    #[snafu(display("failed to read Local Lock passphrase"))]
    Prompt { source: std::io::Error },

    /// An empty passphrase cannot protect Account data.
    #[snafu(display("Local Lock passphrase must not be empty"))]
    EmptyPassphrase,

    /// Initial Local Lock confirmation did not match.
    #[snafu(display("Local Lock passphrases did not match; Account data was not changed"))]
    PassphraseMismatch,

    /// Secure random key generation failed.
    #[snafu(display("failed to generate Local Lock key"))]
    GenerateKey { source: getrandom::Error },

    /// The operating-system credential vault was unavailable.
    #[snafu(display("failed to access the operating-system credential vault"))]
    Keyring { source: keyring::Error },

    /// A credential-vault entry was not an Intuigram key.
    #[snafu(display("operating-system credential vault returned an invalid Local Lock key"))]
    InvalidKeyringKey,

    /// A passphrase salt could not be persisted or loaded.
    #[snafu(display("failed to access Local Lock passphrase salt {}", path.display()))]
    PassphraseSalt { path: PathBuf, source: io::Error },

    /// A persisted passphrase salt has an unsupported length.
    #[snafu(display("Local Lock passphrase salt at {} is invalid", path.display()))]
    InvalidPassphraseSalt { path: PathBuf },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingMaterial {
    None,
    Keyring,
    PassphraseSalt,
}

pub(super) struct LocalUnlock {
    cipher: AccountCipher,
    pending_material: PendingMaterial,
}

impl LocalUnlock {
    pub(super) fn cipher(&self) -> AccountCipher {
        self.cipher.clone()
    }

    pub(super) fn promote(&mut self, config: &Config, account: AccountId) -> Result<(), Error> {
        match self.pending_material {
            PendingMaterial::None => return Ok(()),
            PendingMaterial::Keyring => promote_keyring(account)?,
            PendingMaterial::PassphraseSalt => {
                promote_salt(&config.paths.data, account)?;
            }
        }
        self.pending_material = PendingMaterial::None;
        Ok(())
    }
}

pub(super) fn unlock_local_lock(
    config: &Config,
    account: Option<AccountId>,
    initializing: bool,
) -> Result<LocalUnlock, Error> {
    if !config.local_lock.enabled {
        return Ok(LocalUnlock {
            cipher: AccountCipher::plaintext(),
            pending_material: PendingMaterial::None,
        });
    }
    match config.local_lock.unlock {
        UnlockMethod::Passphrase => {
            let passphrase = Zeroizing::new(
                rpassword::prompt_password("Local Lock passphrase (hidden): ")
                    .context(PromptSnafu)?,
            );
            if passphrase.is_empty() {
                return EmptyPassphraseSnafu.fail();
            }
            if initializing {
                let confirmation = Zeroizing::new(
                    rpassword::prompt_password("Confirm Local Lock passphrase: ")
                        .context(PromptSnafu)?,
                );
                if passphrase.as_str() != confirmation.as_str() {
                    return PassphraseMismatchSnafu.fail();
                }
            }
            let (salt, pending) = passphrase_salt(&config.paths.data, account, initializing)?;
            let mut key = Zeroizing::new([0_u8; 32]);
            pbkdf2_hmac::<Sha256>(passphrase.as_bytes(), &salt, PASSPHRASE_ROUNDS, &mut *key);
            Ok(LocalUnlock {
                cipher: AccountCipher::encrypted(*key),
                pending_material: if pending {
                    PendingMaterial::PassphraseSalt
                } else {
                    PendingMaterial::None
                },
            })
        }
        UnlockMethod::Keyring => keyring_unlock(account, initializing),
    }
}

fn keyring_unlock(account: Option<AccountId>, initializing: bool) -> Result<LocalUnlock, Error> {
    let name = account.map_or_else(|| "pending".to_owned(), |id| id.get().to_string());
    let entry = keyring::Entry::new(KEYRING_SERVICE, &name).context(KeyringSnafu)?;
    let (key, pending) = match entry.get_secret() {
        Ok(secret) => (Zeroizing::new(secret), false),
        Err(keyring::Error::NoEntry) if account.is_some() && !initializing => {
            let pending = keyring::Entry::new(KEYRING_SERVICE, "pending").context(KeyringSnafu)?;
            (
                Zeroizing::new(pending.get_secret().context(KeyringSnafu)?),
                true,
            )
        }
        Err(keyring::Error::NoEntry) => {
            let mut key = Zeroizing::new([0_u8; 32]);
            getrandom::fill(&mut *key).context(GenerateKeySnafu)?;
            entry.set_secret(&*key).context(KeyringSnafu)?;
            (Zeroizing::new(key.to_vec()), account.is_none())
        }
        Err(source) => return Err(Error::Keyring { source }),
    };
    let key =
        Zeroizing::new(<[u8; 32]>::try_from(key.as_slice()).map_err(|_| Error::InvalidKeyringKey)?);
    Ok(LocalUnlock {
        cipher: AccountCipher::encrypted(*key),
        pending_material: if pending {
            PendingMaterial::Keyring
        } else {
            PendingMaterial::None
        },
    })
}

fn promote_keyring(account: AccountId) -> Result<(), Error> {
    let pending = keyring::Entry::new(KEYRING_SERVICE, "pending").context(KeyringSnafu)?;
    let secret = Zeroizing::new(pending.get_secret().context(KeyringSnafu)?);
    keyring::Entry::new(KEYRING_SERVICE, &account.get().to_string())
        .context(KeyringSnafu)?
        .set_secret(&secret)
        .context(KeyringSnafu)?;
    pending.delete_credential().context(KeyringSnafu)
}

fn passphrase_salt(
    data: &Path,
    account: Option<AccountId>,
    initializing: bool,
) -> Result<(Vec<u8>, bool), Error> {
    let path = salt_path(data, account);
    if path.exists() {
        return read_salt(&path).map(|salt| (salt, false));
    }
    if account.is_some() && !initializing {
        let pending = salt_path(data, None);
        if pending.exists() {
            return read_salt(&pending).map(|salt| (salt, true));
        }
        return Ok((LEGACY_PASSPHRASE_SALT.to_vec(), false));
    }
    fs::create_dir_all(data).map_err(|source| Error::PassphraseSalt {
        path: data.to_path_buf(),
        source,
    })?;
    let mut salt = vec![0_u8; SALT_BYTES];
    getrandom::fill(&mut salt).context(GenerateKeySnafu)?;
    write_salt(&path, &salt)?;
    Ok((salt, account.is_none()))
}

fn salt_path(data: &Path, account: Option<AccountId>) -> PathBuf {
    account.map_or_else(
        || data.join(".pending.local-lock-salt"),
        |account| data.join(format!("{}.local-lock-salt", account.get())),
    )
}

fn read_salt(path: &Path) -> Result<Vec<u8>, Error> {
    let salt = fs::read(path).map_err(|source| Error::PassphraseSalt {
        path: path.to_path_buf(),
        source,
    })?;
    if salt.len() != SALT_BYTES {
        return InvalidPassphraseSaltSnafu {
            path: path.to_path_buf(),
        }
        .fail();
    }
    Ok(salt)
}

fn write_salt(path: &Path, salt: &[u8]) -> Result<(), Error> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|source| Error::PassphraseSalt {
        path: path.to_path_buf(),
        source,
    })?;
    file.write_all(salt)
        .map_err(|source| Error::PassphraseSalt {
            path: path.to_path_buf(),
            source,
        })?;
    file.sync_all().map_err(|source| Error::PassphraseSalt {
        path: path.to_path_buf(),
        source,
    })
}

fn promote_salt(data: &Path, account: AccountId) -> Result<(), Error> {
    let pending = salt_path(data, None);
    let target = salt_path(data, Some(account));
    if target.exists() {
        if read_salt(&target)? != read_salt(&pending)? {
            return InvalidPassphraseSaltSnafu { path: target }.fail();
        }
        return fs::remove_file(&pending).map_err(|source| Error::PassphraseSalt {
            path: pending,
            source,
        });
    }
    fs::rename(&pending, &target).map_err(|source| Error::PassphraseSalt {
        path: target,
        source,
    })
}

pub(super) fn delete_local_lock_key(config: &Config, account: AccountId) -> Result<(), Error> {
    if !config.local_lock.enabled || config.local_lock.unlock != UnlockMethod::Keyring {
        return Ok(());
    }
    let entry =
        keyring::Entry::new(KEYRING_SERVICE, &account.get().to_string()).context(KeyringSnafu)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(source) => Err(Error::Keyring { source }),
    }
}

#[cfg(test)]
mod tests {
    use intuigram_store::AccountId;
    use tempfile::tempdir;

    use super::{SALT_BYTES, passphrase_salt, promote_salt, read_salt, salt_path};

    #[test]
    fn new_passphrase_salts_are_random_and_promote_after_login() {
        let temporary = tempdir().expect("temporary directory should be created");
        let account = AccountId::new(42).expect("fixture account should be valid");

        let (salt, pending) =
            passphrase_salt(temporary.path(), None, true).expect("pending salt should be created");

        assert_eq!(salt.len(), SALT_BYTES);
        assert!(pending);
        promote_salt(temporary.path(), account).expect("pending salt should promote");
        assert!(!salt_path(temporary.path(), None).exists());
        assert_eq!(
            read_salt(&salt_path(temporary.path(), Some(account)))
                .expect("promoted salt should load"),
            salt
        );
    }

    #[test]
    fn interrupted_passphrase_salt_promotion_is_recoverable() {
        let temporary = tempdir().expect("temporary directory should be created");
        let account = AccountId::new(43).expect("fixture account should be valid");
        let (salt, _) =
            passphrase_salt(temporary.path(), None, true).expect("pending salt should be created");
        std::fs::write(salt_path(temporary.path(), Some(account)), &salt)
            .expect("promoted salt fixture should be written");

        promote_salt(temporary.path(), account).expect("duplicate promotion should finish");

        assert!(!salt_path(temporary.path(), None).exists());
    }
}
