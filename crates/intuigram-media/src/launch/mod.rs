use std::ffi::OsStr;
use std::path::Path;
use std::process::Stdio;

use compio::process::Command;
use snafu::{ResultExt, Snafu};

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    #[snafu(display("failed to open {target}"))]
    Open {
        target: String,
        source: std::io::Error,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenDisposition {
    OpenAssociatedApplication,
    RevealWithLaunchWarning,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PlatformLauncher;

impl PlatformLauncher {
    pub async fn open_url(self, url: &str) -> Result<()> {
        run(open_command(url), url).await
    }

    pub async fn open_file(self, path: &Path) -> Result<()> {
        let target = path.display().to_string();
        run(open_command(&target), &target).await
    }

    pub async fn reveal_file(self, path: &Path) -> Result<()> {
        let target = path.display().to_string();
        run(reveal_command(path), &target).await
    }
}

async fn run((program, arguments): (&'static str, Vec<String>), target: &str) -> Result<()> {
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
    command.output().await.context(OpenSnafu {
        target: target.to_owned(),
    })?;
    Ok(())
}

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

#[cfg(target_os = "macos")]
fn open_command(target: &str) -> (&'static str, Vec<String>) {
    ("open", vec![target.to_owned()])
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_command(target: &str) -> (&'static str, Vec<String>) {
    ("xdg-open", vec![target.to_owned()])
}

#[cfg(windows)]
fn open_command(target: &str) -> (&'static str, Vec<String>) {
    ("explorer", vec![target.to_owned()])
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{OpenDisposition, open_disposition};

    #[test]
    fn launchable_downloads_are_revealed_instead_of_executed() {
        assert_eq!(
            open_disposition(Path::new("installer.sh"), None),
            OpenDisposition::RevealWithLaunchWarning
        );
        assert_eq!(
            open_disposition(Path::new("manual.pdf"), Some("application/pdf")),
            OpenDisposition::OpenAssociatedApplication
        );
    }
}
