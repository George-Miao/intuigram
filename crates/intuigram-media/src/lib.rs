//! Collision-safe media download and launch-safety policy.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use compio::fs::{OpenOptions, create_dir_all, remove_file};
use compio_io::AsyncWriteAtExt;
use snafu::{ResultExt, Snafu};

/// Failure while materializing a Telegram download.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    /// The configured download directory could not be created.
    #[snafu(display("failed to create download directory {}", path.display()))]
    CreateDirectory {
        /// Configured directory.
        path: PathBuf,

        /// Underlying filesystem failure.
        source: std::io::Error,
    },

    /// A collision-safe output file could not be reserved.
    #[snafu(display("failed to reserve download path {}", path.display()))]
    ReserveFile {
        /// Candidate path.
        path: PathBuf,

        /// Underlying filesystem failure.
        source: std::io::Error,
    },

    /// Download bytes could not be written completely.
    #[snafu(display("failed to write download {}", path.display()))]
    WriteFile {
        /// Reserved download path.
        path: PathBuf,

        /// Underlying filesystem failure.
        source: std::io::Error,
    },

    /// The collision suffix space was exhausted.
    #[snafu(display("could not find a collision-safe filename for {name}"))]
    NamesExhausted {
        /// Sanitized requested filename.
        name: String,
    },
}

/// Result returned by media lifecycle operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Safe action offered after a download completes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenDisposition {
    /// The OS-associated application may open this non-launchable content.
    OpenAssociatedApplication,

    /// Reveal the file in its containing directory with an explicit warning.
    RevealWithLaunchWarning,
}

/// Configured default download destination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownloadDirectory {
    root: PathBuf,
}

impl DownloadDirectory {
    /// Creates a download destination without touching the filesystem.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Atomically reserves a collision-safe name and writes owned bytes.
    pub async fn save(&self, suggested_name: &str, bytes: Vec<u8>) -> Result<PathBuf> {
        create_dir_all(&self.root)
            .await
            .context(CreateDirectorySnafu {
                path: self.root.clone(),
            })?;
        let safe_name = sanitize_filename(suggested_name);
        for suffix in 0..10_000_u32 {
            let candidate = self.root.join(suffixed_name(&safe_name, suffix));
            let opened = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate)
                .await;
            let mut file = match opened {
                Ok(file) => file,
                Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(source) => {
                    return Err(Error::ReserveFile {
                        path: candidate,
                        source,
                    });
                }
            };
            let (written, _) = file.write_all_at(bytes, 0).await.into_parts();
            if let Err(source) = written {
                let _ = remove_file(&candidate).await;
                return Err(Error::WriteFile {
                    path: candidate,
                    source,
                });
            }
            return Ok(candidate);
        }
        NamesExhaustedSnafu { name: safe_name }.fail()
    }
}

/// Chooses whether a downloaded file may be opened or only revealed.
#[must_use]
pub fn open_disposition(path: &Path, mime_type: Option<&str>) -> OpenDisposition {
    if mime_type.is_some_and(|mime| {
        mime == "application/x-executable"
            || mime == "application/x-sharedlib"
            || mime == "application/x-msdownload"
            || mime == "application/x-shellscript"
    }) || path
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "app"
                    | "appimage"
                    | "bat"
                    | "cmd"
                    | "com"
                    | "desktop"
                    | "dll"
                    | "dmg"
                    | "exe"
                    | "jar"
                    | "msi"
                    | "ps1"
                    | "scr"
                    | "sh"
            )
        })
    {
        OpenDisposition::RevealWithLaunchWarning
    } else {
        OpenDisposition::OpenAssociatedApplication
    }
}

fn sanitize_filename(name: &str) -> String {
    let file = Path::new(name)
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("download");
    let sanitized = file
        .chars()
        .map(|character| {
            if character.is_control() || matches!(character, '/' | '\\' | ':' | '\0') {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    if sanitized.trim_matches(['.', ' ']).is_empty() {
        "download".to_owned()
    } else {
        sanitized
    }
}

fn suffixed_name(name: &str, suffix: u32) -> String {
    if suffix == 0 {
        return name.to_owned();
    }
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("download");
    path.extension().and_then(OsStr::to_str).map_or_else(
        || format!("{stem} ({suffix})"),
        |extension| format!("{stem} ({suffix}).{extension}"),
    )
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{DownloadDirectory, OpenDisposition, open_disposition};

    #[test]
    fn downloads_never_overwrite_an_existing_name() {
        let temporary = tempdir().expect("temporary directory should be created");
        fs::write(temporary.path().join("report.txt"), b"existing")
            .expect("collision fixture should be written");
        let runtime = compio::runtime::Runtime::new().expect("runtime should initialize");
        let path = runtime
            .block_on(DownloadDirectory::new(temporary.path()).save("report.txt", b"new".to_vec()))
            .expect("download should use a collision suffix");

        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("report (1).txt")
        );
        assert_eq!(fs::read(path).expect("download should be readable"), b"new");
        assert_eq!(
            fs::read(temporary.path().join("report.txt")).expect("fixture should remain"),
            b"existing"
        );
    }

    #[test]
    fn launchable_downloads_are_revealed_instead_of_executed() {
        assert_eq!(
            open_disposition(std::path::Path::new("installer.sh"), None),
            OpenDisposition::RevealWithLaunchWarning
        );
        assert_eq!(
            open_disposition(std::path::Path::new("manual.pdf"), Some("application/pdf")),
            OpenDisposition::OpenAssociatedApplication
        );
    }
}
