//! Collision-safe media downloads and platform launch safety.

mod cache;
mod download;
mod launch;
mod preview;

pub use cache::{CacheKey, CacheKind, CacheUsage, Error as CacheError, MediaCache};
pub use download::{DownloadDirectory, Error as DownloadError};
pub use launch::{Error as LaunchError, OpenDisposition, PlatformLauncher, open_disposition};
pub use preview::{Error as PreviewError, decode_preview};
