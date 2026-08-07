//! First-open Local Lock acquisition without exposing secrets to app state.

use intuigram_config::{Config, UnlockMethod};
use intuigram_store::{AccountCipher, AccountId};
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use snafu::{ResultExt, Snafu};
use zeroize::Zeroizing;

const KEYRING_SERVICE: &str = "dev.intuigram.local-lock";
const PASSPHRASE_ROUNDS: u32 = 600_000;

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
}

pub(super) struct LocalUnlock {
    cipher: AccountCipher,
    pending_keyring: bool,
}

impl LocalUnlock {
    pub(super) fn cipher(&self) -> AccountCipher {
        self.cipher.clone()
    }

    pub(super) fn promote_keyring(&mut self, account: AccountId) -> Result<(), Error> {
        if !self.pending_keyring {
            return Ok(());
        }
        let pending = keyring::Entry::new(KEYRING_SERVICE, "pending").context(KeyringSnafu)?;
        let secret = pending.get_secret().context(KeyringSnafu)?;
        keyring::Entry::new(KEYRING_SERVICE, &account.get().to_string())
            .context(KeyringSnafu)?
            .set_secret(&secret)
            .context(KeyringSnafu)?;
        pending.delete_credential().context(KeyringSnafu)?;
        self.pending_keyring = false;
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
            pending_keyring: false,
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
            let mut key = [0_u8; 32];
            pbkdf2_hmac::<Sha256>(
                passphrase.as_bytes(),
                b"Intuigram Local Lock v1",
                PASSPHRASE_ROUNDS,
                &mut key,
            );
            Ok(LocalUnlock {
                cipher: AccountCipher::encrypted(key),
                pending_keyring: false,
            })
        }
        UnlockMethod::Keyring => keyring_unlock(account),
    }
}

fn keyring_unlock(account: Option<AccountId>) -> Result<LocalUnlock, Error> {
    let name = account.map_or_else(|| "pending".to_owned(), |id| id.get().to_string());
    let entry = keyring::Entry::new(KEYRING_SERVICE, &name).context(KeyringSnafu)?;
    let key = Zeroizing::new(match entry.get_secret() {
        Ok(secret) => secret,
        Err(keyring::Error::NoEntry) => {
            let mut key = [0_u8; 32];
            getrandom::fill(&mut key).context(GenerateKeySnafu)?;
            entry.set_secret(&key).context(KeyringSnafu)?;
            key.to_vec()
        }
        Err(source) => return Err(Error::Keyring { source }),
    });
    let key: [u8; 32] = key
        .as_slice()
        .try_into()
        .map_err(|_| Error::InvalidKeyringKey)?;
    Ok(LocalUnlock {
        cipher: AccountCipher::encrypted(key),
        pending_keyring: account.is_none(),
    })
}
