use std::path::PathBuf;

use snafu::Snafu;

/// Failure while presenting or carrying out Account recovery.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(super)))]
pub enum Error {
    /// The recovery screen could not be drawn.
    #[snafu(display("failed to draw Account recovery"))]
    Draw { source: intuigram_tui::Error },

    /// Terminal input stopped during recovery.
    #[snafu(display("failed to read Account recovery input"))]
    Input { source: intuigram_tui::Error },

    /// A normal Account-open retry failed before it could be classified.
    #[snafu(display("failed to retry Account database recovery"))]
    Retry { source: intuigram_store::Error },

    /// A verified cache rebuild could not be completed safely.
    #[snafu(display("failed to rebuild synchronized Account cache"))]
    Rebuild {
        source: intuigram_store::RecoveryError,
    },

    /// The platform helper for revealing a backup could not be run.
    #[snafu(display("failed to open backup location for {}", path.display()))]
    OpenBackupLocation {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Result of the startup Account recovery workflow.
pub type Result<T, E = Error> = std::result::Result<T, E>;
