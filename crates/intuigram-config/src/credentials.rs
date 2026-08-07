//! First-run persistence for user-owned Telegram application credentials.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use snafu::{ResultExt, Snafu};

/// Failure while validating or saving first-run credentials.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    /// Telegram application IDs must be positive.
    #[snafu(display("Telegram application ID must be positive"))]
    InvalidApplicationId,

    /// Telegram application hashes are fixed hexadecimal tokens.
    #[snafu(display("Telegram application hash must contain exactly 32 hexadecimal characters"))]
    InvalidApplicationHash,

    /// The platform configuration directory could not be created.
    #[snafu(display("failed to create configuration directory {}", path.display()))]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },

    /// An existing credential file is never overwritten by onboarding.
    #[snafu(display("credential file already exists at {}", path.display()))]
    AlreadyExists { path: PathBuf },

    /// Credential bytes could not be written.
    #[snafu(display("failed to write credential file {}", path.display()))]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },

    /// Owner-only permissions could not be applied.
    #[snafu(display("failed to protect credential file {}", path.display()))]
    Protect {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Saves user-owned credentials into a dedicated, non-overwriting source.
pub fn save_application_credentials(
    config_directory: &Path,
    api_id: i32,
    api_hash: &str,
) -> Result<PathBuf, Error> {
    if api_id <= 0 {
        return InvalidApplicationIdSnafu.fail();
    }
    if api_hash.len() != 32 || !api_hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return InvalidApplicationHashSnafu.fail();
    }
    fs::create_dir_all(config_directory).context(CreateDirectorySnafu {
        path: config_directory.to_path_buf(),
    })?;
    let path = config_directory.join("credentials.toml");
    let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(file) => file,
        Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
            return AlreadyExistsSnafu { path }.fail();
        }
        Err(source) => {
            return Err(Error::Write { path, source });
        }
    };
    let write = (|| {
        protect(&path)?;
        writeln!(
            file,
            "# User-owned Telegram application credentials. Never commit this \
             file.\n[telegram]\napi_id = {api_id}\napi_hash = \"{api_hash}\""
        )
        .context(WriteSnafu { path: path.clone() })?;
        file.sync_all().context(WriteSnafu { path: path.clone() })
    })();
    if let Err(error) = write {
        let _ = fs::remove_file(&path);
        return Err(error);
    }
    Ok(path)
}

#[cfg(unix)]
fn protect(path: &Path) -> Result<(), Error> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).context(ProtectSnafu {
        path: path.to_path_buf(),
    })
}

#[cfg(not(unix))]
fn protect(_path: &Path) -> Result<(), Error> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::save_application_credentials;
    use crate::{ConfigLoader, PlatformDefaults};

    #[test]
    fn first_run_credentials_are_private_loadable_and_never_overwritten() {
        let temporary = tempdir().expect("temporary config root should be created");
        let config = temporary.path().join("config");
        let path = save_application_credentials(&config, 42, "0123456789abcdef0123456789abcdef")
            .expect("credentials should be saved");
        let loaded = ConfigLoader::new(PlatformDefaults {
            config,
            data: temporary.path().join("data"),
            cache: temporary.path().join("cache"),
            downloads: temporary.path().join("downloads"),
        })
        .read_environment(false)
        .load()
        .expect("saved credentials should load");

        assert_eq!(loaded.telegram.api_id, Some(42));
        assert_eq!(
            loaded.telegram.api_hash.as_ref().map(|hash| hash.expose()),
            Some("0123456789abcdef0123456789abcdef")
        );
        assert!(
            save_application_credentials(
                path.parent().expect("credential path has a parent"),
                43,
                "abcdef0123456789abcdef0123456789",
            )
            .is_err()
        );
        assert!(
            fs::read_to_string(path)
                .expect("credentials should remain")
                .contains("api_id = 42")
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mode = fs::metadata(temporary.path().join("config/credentials.toml"))
                .expect("credential metadata should be readable")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
    }
}
