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

    /// The native clipboard could not provide its content.
    #[cfg(target_os = "macos")]
    #[snafu(display("failed to read the native clipboard"))]
    ReadNative { source: arboard::Error },

    /// Native image dimensions did not match its RGBA bytes.
    #[cfg(target_os = "macos")]
    #[snafu(display(
        "clipboard image dimensions {width} by {height} do not match {bytes} RGBA bytes"
    ))]
    InvalidImage {
        width: usize,
        height: usize,
        bytes: usize,
    },

    /// A native clipboard image could not be encoded as PNG.
    #[cfg(target_os = "macos")]
    #[snafu(display("failed to encode the clipboard image as PNG"))]
    EncodeImage { source: image::ImageError },

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
    use std::io::Cursor;
    use std::path::PathBuf;

    use compio::runtime::ResumeUnwind;
    use image::{DynamicImage, ImageFormat, RgbaImage};
    use snafu::ResultExt;

    use super::{
        ClipboardContent, ClipboardSnapshot, EncodeImageSnafu, InvalidImageSnafu, ReadNativeSnafu,
        Result, output,
    };

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

        compio::runtime::spawn_blocking(read_native)
            .await
            .resume_unwind()
            .expect("an awaited native clipboard read cannot be cancelled")
    }

    fn read_native() -> Result<ClipboardContent> {
        let mut clipboard = arboard::Clipboard::new().context(ReadNativeSnafu)?;
        match clipboard.get_image() {
            Ok(image) => {
                let png = encode_png(
                    image.width,
                    image.height,
                    image.into_owned_bytes().into_owned(),
                )?;
                return Ok(ClipboardContent::Image {
                    mime_type: "image/png".to_owned(),
                    bytes: png,
                });
            }
            Err(arboard::Error::ContentNotAvailable) => {}
            Err(source) => return Err(source).context(ReadNativeSnafu),
        }
        match clipboard.get_text() {
            Ok(text) if !text.is_empty() => Ok(ClipboardContent::Text(text)),
            Ok(_) | Err(arboard::Error::ContentNotAvailable) => {
                ClipboardSnapshot::default().resolve()
            }
            Err(source) => Err(source).context(ReadNativeSnafu),
        }
    }

    pub(super) fn encode_png(width: usize, height: usize, bytes: Vec<u8>) -> Result<Vec<u8>> {
        let byte_count = bytes.len();
        let image = u32::try_from(width)
            .ok()
            .zip(u32::try_from(height).ok())
            .and_then(|(width, height)| RgbaImage::from_raw(width, height, bytes));
        let Some(image) = image else {
            return InvalidImageSnafu {
                width,
                height,
                bytes: byte_count,
            }
            .fail();
        };
        let mut png = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut png, ImageFormat::Png)
            .context(EncodeImageSnafu)?;
        Ok(png.into_inner())
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

    #[cfg(target_os = "macos")]
    #[test]
    fn native_rgba_image_encodes_valid_png() {
        let png = super::sys::encode_png(1, 1, vec![0x11, 0x22, 0x33, 0xff])
            .expect("one RGBA pixel should encode");
        let decoded = image::load_from_memory(&png).expect("encoded clipboard image should decode");

        assert_eq!(decoded.width(), 1);
        assert_eq!(decoded.height(), 1);
        assert_eq!(decoded.to_rgba8().as_raw(), &[0x11, 0x22, 0x33, 0xff]);
    }
}
