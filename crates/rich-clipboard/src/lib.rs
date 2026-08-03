//! Native, command-backed rich clipboard acquisition.

use std::path::PathBuf;
use std::process::Stdio;

use compio::process::Command;
use snafu::{ResultExt, Snafu};

/// Rich clipboard value in precedence order: files, image, then text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipboardContent {
    /// Unicode text suitable for insertion into a Draft.
    Text(String),

    /// Encoded image bytes suitable for a photo attachment candidate.
    Image {
        /// Internet media type.
        mime_type: String,

        /// Encoded image bytes.
        bytes: Vec<u8>,
    },

    /// Native filesystem items suitable for file attachment candidates.
    Files(Vec<PathBuf>),
}

/// Native clipboard representations gathered by a platform adapter.
///
/// Resolving a snapshot applies the crate-wide format precedence without
/// exposing platform commands to callers.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ClipboardSnapshot {
    /// Native filesystem items.
    pub files: Vec<PathBuf>,

    /// PNG representation, when available.
    pub png: Option<Vec<u8>>,

    /// Unicode text representation, when available.
    pub text: Option<String>,
}

impl ClipboardSnapshot {
    /// Resolves the richest supported representation using files, image, then
    /// text precedence.
    pub fn resolve(self) -> Result<ClipboardContent> {
        if !self.files.is_empty() {
            return Ok(ClipboardContent::Files(self.files));
        }
        if let Some(bytes) = self.png.filter(|bytes| !bytes.is_empty()) {
            return Ok(ClipboardContent::Image {
                mime_type: "image/png".to_owned(),
                bytes,
            });
        }
        if let Some(text) = self.text.filter(|text| !text.is_empty()) {
            return Ok(ClipboardContent::Text(text));
        }
        UnsupportedContentSnafu.fail()
    }
}

/// Failure while querying the native clipboard.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    /// This platform has no implemented clipboard adapter.
    #[snafu(display("native rich clipboard integration is unavailable on this platform"))]
    UnsupportedPlatform,

    /// A platform clipboard helper could not be launched or awaited.
    #[snafu(display("failed to query clipboard through {program}"))]
    RunHelper {
        /// Helper program.
        program: &'static str,

        /// Underlying process failure.
        source: std::io::Error,
    },

    /// The clipboard has no supported file, image, or text representation.
    #[snafu(display("clipboard does not contain supported files, image data, or text"))]
    UnsupportedContent,
}

/// Result returned by clipboard operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Queries the native clipboard without blocking the Compio runtime thread.
pub async fn read() -> Result<ClipboardContent> {
    sys::read().await
}

async fn output(program: &'static str, arguments: &[&str]) -> Result<std::process::Output> {
    let mut command = Command::new(program);
    command.args(arguments);
    command
        .stdin(Stdio::null())
        .expect("std::process::Stdio conversion is infallible");
    command
        .stdout(Stdio::piped())
        .expect("std::process::Stdio conversion is infallible");
    command
        .stderr(Stdio::null())
        .expect("std::process::Stdio conversion is infallible");
    command.output().await.context(RunHelperSnafu { program })
}

#[cfg(target_os = "macos")]
mod sys {
    use std::path::PathBuf;

    use super::{ClipboardContent, ClipboardSnapshot, Result, output};

    const FILE_SCRIPT: &str = "try\nset xs to the clipboard as list\nset out to \"\"\nrepeat with \
                               x in xs\nset out to out & POSIX path of x & linefeed\nend \
                               repeat\nreturn out\non error\nreturn \"\"\nend try";

    pub async fn read() -> Result<ClipboardContent> {
        let files = output("osascript", &["-e", FILE_SCRIPT]).await?;
        if files.status.success() {
            let paths = String::from_utf8_lossy(&files.stdout)
                .lines()
                .filter(|line| !line.is_empty())
                .map(PathBuf::from)
                .collect::<Vec<_>>();
            if !paths.is_empty() {
                return ClipboardSnapshot {
                    files: paths,
                    ..ClipboardSnapshot::default()
                }
                .resolve();
            }
        }

        if let Ok(image) = output("pngpaste", &["-"]).await
            && image.status.success()
            && !image.stdout.is_empty()
        {
            return ClipboardSnapshot {
                png: Some(image.stdout),
                ..ClipboardSnapshot::default()
            }
            .resolve();
        }

        let text = output("pbpaste", &[]).await?;
        if text.status.success() && !text.stdout.is_empty() {
            return ClipboardSnapshot {
                text: Some(String::from_utf8_lossy(&text.stdout).into_owned()),
                ..ClipboardSnapshot::default()
            }
            .resolve();
        }
        ClipboardSnapshot::default().resolve()
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod sys {
    use std::path::PathBuf;

    use super::{ClipboardContent, ClipboardSnapshot, Result, output};

    pub async fn read() -> Result<ClipboardContent> {
        let types = output("wl-paste", &["--list-types"]).await?;
        let types = String::from_utf8_lossy(&types.stdout);
        let mut snapshot = ClipboardSnapshot::default();
        if types.lines().any(|kind| kind == "text/uri-list") {
            let files = output("wl-paste", &["--type", "text/uri-list"]).await?;
            snapshot.files = String::from_utf8_lossy(&files.stdout)
                .lines()
                .filter_map(|line| line.strip_prefix("file://"))
                .map(PathBuf::from)
                .collect::<Vec<_>>();
        }
        if types.lines().any(|kind| kind == "image/png") {
            let image = output("wl-paste", &["--type", "image/png"]).await?;
            if !image.stdout.is_empty() {
                snapshot.png = Some(image.stdout);
            }
        }
        if types.lines().any(|kind| kind.starts_with("text/plain")) {
            let text = output("wl-paste", &["--type", "text/plain;charset=utf-8"]).await?;
            if !text.stdout.is_empty() {
                snapshot.text = Some(String::from_utf8_lossy(&text.stdout).into_owned());
            }
        }
        snapshot.resolve()
    }
}

#[cfg(not(unix))]
mod sys {
    use super::{ClipboardContent, Result, UnsupportedPlatformSnafu};

    pub async fn read() -> Result<ClipboardContent> {
        UnsupportedPlatformSnafu.fail()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{ClipboardContent, ClipboardSnapshot};

    #[test]
    fn files_take_precedence_over_image_and_text_representations() {
        let content = ClipboardSnapshot {
            files: vec![PathBuf::from("photo.heic")],
            png: Some(vec![1, 2, 3]),
            text: Some("caption".to_owned()),
        }
        .resolve()
        .expect("a supported representation should resolve");

        assert_eq!(
            content,
            ClipboardContent::Files(vec![PathBuf::from("photo.heic")])
        );
    }

    #[test]
    fn image_precedes_text_and_empty_snapshots_are_rejected() {
        assert_eq!(
            ClipboardSnapshot {
                files: Vec::new(),
                png: Some(vec![4, 5, 6]),
                text: Some("caption".to_owned()),
            }
            .resolve()
            .expect("PNG should resolve"),
            ClipboardContent::Image {
                mime_type: "image/png".to_owned(),
                bytes: vec![4, 5, 6],
            }
        );
        assert!(ClipboardSnapshot::default().resolve().is_err());
    }
}
