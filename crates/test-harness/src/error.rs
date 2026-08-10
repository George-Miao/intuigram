//! Harness-owned failures and trace artifact references.

use std::path::PathBuf;

use intuigram_lib::Action;
use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(super)))]
pub enum Error {
    #[snafu(display("the behavior test did not configure a Telegram scenario"))]
    MissingTelegramScenario,

    #[snafu(display("failed to initialize the behavior-test database"))]
    Store { source: intuigram_store::Error },

    #[snafu(display("failed to commit a behavior-test Telegram update"))]
    Sync { source: intuigram_app::SyncError },

    #[snafu(display("failed to create isolated behavior-test roots"))]
    CreateRoots { source: std::io::Error },

    #[snafu(display("failed to write behavior-test media {}", path.display()))]
    WriteMedia {
        path: PathBuf,

        source: std::io::Error,
    },

    #[snafu(display("terminal input {event} is unavailable in the current UI context{artifact}"))]
    UnavailableInput { event: String, artifact: Artifact },

    #[snafu(display("unexpected application effect: {effect}{artifact}"))]
    UnexpectedEffect { effect: String, artifact: Artifact },

    #[snafu(display(
        "Telegram scenario mismatch: expected {expected}, observed {observed}{artifact}"
    ))]
    TelegramMismatch {
        expected: String,
        observed: String,
        artifact: Artifact,
    },

    #[snafu(display("Telegram scenario still has pending work: {work}{artifact}"))]
    PendingWork { work: String, artifact: Artifact },

    #[snafu(display(
        "semantic locator {query:?} matched {matches} nodes, expected exactly one{artifact}"
    ))]
    LocatorCardinality {
        query: String,
        matches: usize,
        artifact: Artifact,
    },

    #[snafu(display("semantic expectation failed: {expectation}; last value: {actual}{artifact}"))]
    Expectation {
        expectation: String,
        actual: String,
        artifact: Artifact,
    },

    #[snafu(display("action {action:?} is not available{artifact}"))]
    ActionUnavailable { action: Action, artifact: Artifact },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Clone, Debug, Default)]
pub struct Artifact(pub Option<PathBuf>);

impl std::fmt::Display for Artifact {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(path) => write!(formatter, "; trace: {}", path.display()),
            None => Ok(()),
        }
    }
}
