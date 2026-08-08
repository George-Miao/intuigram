use std::sync::Arc;

use snafu::Snafu;

/// Invalid owned RGBA image data.
#[derive(Debug, Snafu)]
pub enum Error {
    /// The dimensions cannot describe a byte payload on this platform.
    #[snafu(display("RGBA image dimensions {width}x{height} overflow"))]
    DimensionsOverflow { width: u32, height: u32 },

    /// The supplied bytes do not contain one RGBA pixel per coordinate.
    #[snafu(display(
        "RGBA image dimensions {width}x{height} require {expected} bytes, got {actual}"
    ))]
    LengthMismatch {
        width: u32,
        height: u32,
        expected: usize,
        actual: usize,
    },
}

/// Small immutable row-major RGBA8 image.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Image {
    width: u32,
    height: u32,
    rgba: Arc<[u8]>,
}

impl Image {
    /// Validates and owns an RGBA8 payload.
    pub fn from_rgba(width: u32, height: u32, rgba: Vec<u8>) -> Result<Self, Error> {
        let expected = usize::try_from(width)
            .ok()
            .and_then(|width| usize::try_from(height).ok().map(|height| (width, height)))
            .and_then(|(width, height)| width.checked_mul(height))
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(Error::DimensionsOverflow { width, height })?;
        if rgba.len() != expected {
            return Err(Error::LengthMismatch {
                width,
                height,
                expected,
                actual: rgba.len(),
            });
        }
        Ok(Self {
            width,
            height,
            rgba: rgba.into(),
        })
    }

    /// Pixel width.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Pixel height.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Immutable RGBA8 pixels.
    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}
