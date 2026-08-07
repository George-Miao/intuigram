//! Collision-safe media downloads and platform launch safety.

mod download;
mod launch;
mod preview;

pub use download::{DownloadDirectory, Error as DownloadError};
pub use launch::{Error as LaunchError, OpenDisposition, PlatformLauncher, open_disposition};
pub use preview::{Error as PreviewError, decode_preview};
