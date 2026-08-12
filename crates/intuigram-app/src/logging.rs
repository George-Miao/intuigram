use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use snafu::{ResultExt, Snafu};

#[derive(Debug, Snafu)]
pub(super) enum Error {
    #[snafu(display("failed to create log directory {}", path.display()))]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("failed to open log file {}", path.display()))]
    OpenFile {
        path: PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("failed to install the process log subscriber"))]
    Install {
        source: tracing::subscriber::SetGlobalDefaultError,
    },
}

pub(super) type Result<T, E = Error> = std::result::Result<T, E>;

pub(super) fn initialize(path: &Path) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).context(CreateDirectorySnafu { path: parent })?;
    }
    let file = open_log(path)?;
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .with_writer(Mutex::new(file))
        .finish();
    tracing::subscriber::set_global_default(subscriber).context(InstallSnafu)?;
    tracing::info!(path = %path.display(), "Intuigram logging initialized");
    Ok(())
}

pub(super) fn connection_interrupted(reason: &str) {
    tracing::warn!(reason, "Telegram connection interrupted");
}

fn open_log(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path).context(OpenFileSnafu { path })
}
