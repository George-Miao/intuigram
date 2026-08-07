use std::io::Cursor;

use image::imageops::FilterType;
use image::{ImageReader, Limits};
use intuigram_app::InlineImage;
use snafu::{ResultExt, Snafu};

const MAX_SOURCE_DIMENSION: u32 = 16_384;
const MAX_DECODED_BYTES: u64 = 64 * 1024 * 1024;
const PREVIEW_WIDTH: u32 = 32;
const PREVIEW_HEIGHT: u32 = 12;

/// Failure while decoding untrusted media bytes into a bounded terminal
/// preview.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    /// The encoded format could not be identified.
    #[snafu(display("failed to identify downloaded image format"))]
    Identify { source: std::io::Error },

    /// The encoded image was invalid, unsupported, or exceeded safety limits.
    #[snafu(display("failed to decode downloaded image preview"))]
    Decode { source: image::ImageError },
}

/// Result returned by terminal-preview decoding.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Decodes the first image frame and bounds it for inexpensive immutable-view
/// cloning.
pub fn decode_preview(encoded: &[u8]) -> Result<InlineImage> {
    let mut reader = ImageReader::new(Cursor::new(encoded))
        .with_guessed_format()
        .context(IdentifySnafu)?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SOURCE_DIMENSION);
    limits.max_image_height = Some(MAX_SOURCE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODED_BYTES);
    reader.limits(limits);
    let decoded = reader.decode().context(DecodeSnafu)?;
    let preview = if decoded.width() > PREVIEW_WIDTH || decoded.height() > PREVIEW_HEIGHT {
        decoded
            .resize(PREVIEW_WIDTH, PREVIEW_HEIGHT, FilterType::Triangle)
            .to_rgba8()
    } else {
        decoded.to_rgba8()
    };
    let width = u16::try_from(preview.width())
        .expect("the fixed preview width always fits in a terminal dimension");
    let height = u16::try_from(preview.height())
        .expect("the fixed preview height always fits in a terminal dimension");
    Ok(InlineImage::from_rgba(width, height, preview.into_raw())
        .expect("the image decoder returns exactly four bytes for every RGBA8 pixel"))
}

#[cfg(test)]
mod tests {
    use super::decode_preview;

    const ONE_PIXEL_PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x04, 0x00, 0x00, 0x00, 0xb5,
        0x1c, 0x0c, 0x02, 0x00, 0x00, 0x00, 0x0b, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x64,
        0xf8, 0x0f, 0x00, 0x01, 0x05, 0x01, 0x01, 0x27, 0x18, 0xe3, 0x66, 0x00, 0x00, 0x00, 0x00,
        0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    #[test]
    fn encoded_image_is_decoded_into_a_bounded_rgba_preview() {
        let preview = decode_preview(ONE_PIXEL_PNG).expect("the PNG fixture should decode");

        assert_eq!(preview.width(), 1);
        assert_eq!(preview.height(), 1);
        assert_eq!(preview.rgba().len(), 4);
    }

    #[test]
    fn malformed_image_is_rejected_without_a_partial_preview() {
        assert!(decode_preview(b"not an image").is_err());
    }
}
