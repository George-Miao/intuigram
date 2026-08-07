use std::path::Path;
use std::process::Stdio;

use compio::process::Command;
use intuigram_store::{AccountDatabase, AccountOpen, AccountRecovery};
use intuigram_tui::{
    RecoveryAction, RecoveryView, TerminalEvents, TerminalUi, resolve_recovery_event,
};
use snafu::ResultExt;

mod error;

use error::{DrawSnafu, InputSnafu, OpenBackupLocationSnafu, RebuildSnafu, RetrySnafu};
pub use error::{Error, Result};

/// User-selected result of a startup recovery workflow.
pub enum Outcome {
    /// A validated or safely rebuilt Account database.
    Ready(AccountDatabase),

    /// The user left without changing Account data.
    Cancelled,
}

/// Keeps the failed database read-only until the user chooses an explicit
/// recovery action.
pub async fn run(
    terminal: &mut TerminalUi,
    events: &mut TerminalEvents,
    recovery: Box<AccountRecovery>,
    account_name: String,
) -> Result<Outcome> {
    let mut recovery = Some(recovery);
    let mut ready = None;
    let mut view = recovery_view(
        recovery
            .as_deref()
            .expect("recovery exists before the first action"),
        account_name,
    );
    loop {
        terminal.draw_recovery(&view).context(DrawSnafu)?;
        let event = events.next_event().await.context(InputSnafu)?;
        let Some(action) = resolve_recovery_event(event, view.rebuild_blocker.is_none()) else {
            continue;
        };
        match action {
            RecoveryAction::Retry => {
                let Some(current) = recovery.take() else {
                    continue;
                };
                match current.retry().context(RetrySnafu)? {
                    AccountOpen::Ready(database) => return Ok(Outcome::Ready(database)),
                    AccountOpen::Recovery(next) => {
                        view = recovery_view(&next, view.account_name.clone());
                        recovery = Some(next);
                    }
                }
            }
            RecoveryAction::OpenBackupLocation => {
                let target = view
                    .backup_paths
                    .last()
                    .unwrap_or(&view.database_path)
                    .clone();
                view.notice = match reveal(&target).await {
                    Ok(()) => Some(format!("Opened {}", target.display())),
                    Err(error) => Some(error.to_string()),
                };
            }
            RecoveryAction::RebuildCache => {
                let Some(current) = recovery.take() else {
                    continue;
                };
                if !current.can_rebuild_cache() {
                    recovery = Some(current);
                    continue;
                }
                let rebuilt = current.rebuild_cache().context(RebuildSnafu)?;
                let backup = rebuilt.preserved_original().to_path_buf();
                view.backup_paths.push(backup.clone());
                view.completion = Some(format!(
                    "Cache rebuilt. Original database preserved at {}",
                    backup.display()
                ));
                view.notice = None;
                ready = Some(rebuilt.into_database());
            }
            RecoveryAction::Continue => {
                if let Some(database) = ready.take() {
                    return Ok(Outcome::Ready(database));
                }
            }
            RecoveryAction::Cancel => return Ok(Outcome::Cancelled),
            RecoveryAction::Redraw => {}
        }
    }
}

fn recovery_view(recovery: &AccountRecovery, account_name: String) -> RecoveryView {
    RecoveryView {
        account_name,
        database_path: recovery.database_path().to_path_buf(),
        backup_paths: recovery.backup_paths().to_vec(),
        failure: recovery.cause().to_string(),
        rebuild_blocker: recovery.rebuild_blocker().map(ToString::to_string),
        completion: None,
        notice: None,
    }
}

async fn reveal(path: &Path) -> Result<()> {
    let (program, arguments): (&'static str, Vec<String>) = reveal_command(path);
    let mut command = Command::new(program);
    command.args(arguments);
    command
        .stdin(Stdio::null())
        .expect("std::process::Stdio conversion is infallible");
    command
        .stdout(Stdio::null())
        .expect("std::process::Stdio conversion is infallible");
    command
        .stderr(Stdio::null())
        .expect("std::process::Stdio conversion is infallible");
    command.output().await.context(OpenBackupLocationSnafu {
        path: path.to_path_buf(),
    })?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn reveal_command(path: &Path) -> (&'static str, Vec<String>) {
    ("open", vec!["-R".to_owned(), path.display().to_string()])
}

#[cfg(all(unix, not(target_os = "macos")))]
fn reveal_command(path: &Path) -> (&'static str, Vec<String>) {
    let directory = path.parent().unwrap_or(path);
    ("xdg-open", vec![directory.display().to_string()])
}

#[cfg(windows)]
fn reveal_command(path: &Path) -> (&'static str, Vec<String>) {
    ("explorer", vec![format!("/select,{}", path.display())])
}

#[cfg(not(any(unix, windows)))]
fn reveal_command(path: &Path) -> (&'static str, Vec<String>) {
    ("", vec![path.display().to_string()])
}
