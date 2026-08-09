//! Direct Ghostty graphics probe independent of Ratatui.
//!
//! Run with `unicode` or `legacy` as the first argument and an image path as
//! the second. The probe remains visible for sixty seconds so an automated
//! screenshot can inspect it.

use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;

use image::imageops::FilterType;
use rasterm::{
    CellBounds, CellPixels, Image, ImageId, Multiplexer, Placement, Protocol, Renderer, fit_cells,
    unicode_placeholder,
};
use snafu::{OptionExt, ResultExt, Snafu};

const IMAGE_ID: u32 = 0x11_22_33;

#[derive(Debug, Snafu)]
enum Error {
    #[snafu(display("failed to render the Ghostty probe image"))]
    Render { source: rasterm::Error },

    #[snafu(display("failed to write the Ghostty probe terminal output"))]
    Write { source: io::Error },

    #[snafu(display("an image path is required for the Ghostty probe"))]
    ImagePath,

    #[snafu(display("failed to decode probe image {}", path.display()))]
    DecodeImage {
        path: PathBuf,
        source: image::ImageError,
    },

    #[snafu(display("failed to validate the decoded probe image"))]
    ValidateImage { source: rasterm::ImageError },
}

type Result<T, E = Error> = std::result::Result<T, E>;

fn main() -> Result<()> {
    let mut arguments = std::env::args().skip(1);
    let protocol = match arguments.next().as_deref() {
        Some("legacy") => Protocol::KittyLegacy,
        _ => Protocol::KittyUnicode,
    };
    let path = arguments
        .next()
        .map(PathBuf::from)
        .context(ImagePathSnafu)?;
    let image = probe_image(path)?;
    let id = ImageId::new(IMAGE_ID).expect("the fixed probe image ID is nonzero");
    let placement = Placement {
        id,
        size: fit_cells(
            image.width(),
            image.height(),
            CellBounds {
                columns: 32,
                rows: 12,
            },
        ),
        image: Arc::new(image),
        cell_pixels: CellPixels::default(),
        x: 2,
        y: 2,
        multiplexer: Multiplexer::None,
    };
    let mut output = b"\x1b[2J\x1b[H".to_vec();
    Renderer::new(protocol)
        .sync(&mut output, std::slice::from_ref(&placement))
        .context(RenderSnafu)?;
    if protocol == Protocol::KittyUnicode {
        append_unicode_placeholders(&mut output, &placement);
    }
    output.extend_from_slice(b"\x1b[16;3HDetailed image probe (closes after 60 seconds)\x1b[0m");
    let mut stdout = io::stdout().lock();
    stdout.write_all(&output).context(WriteSnafu)?;
    stdout.flush().context(WriteSnafu)?;
    std::thread::sleep(std::time::Duration::from_secs(60));
    Ok(())
}

fn append_unicode_placeholders(output: &mut Vec<u8>, placement: &Placement) {
    let id = placement.id.get();
    let (red, green, blue) = ((id >> 16) & 0xff, (id >> 8) & 0xff, id & 0xff);
    output.extend_from_slice(format!("\x1b[38;2;{red};{green};{blue}m").as_bytes());
    for row in 0..placement.size.rows {
        output.extend_from_slice(
            format!("\x1b[{};{}H", placement.y + row + 1, placement.x + 1).as_bytes(),
        );
        for column in 0..placement.size.columns {
            output.extend_from_slice(
                unicode_placeholder(row, column)
                    .expect("the probe fits the supported placeholder coordinates")
                    .as_bytes(),
            );
        }
    }
}

fn probe_image(path: PathBuf) -> Result<Image> {
    let decoded = image::open(&path)
        .context(DecodeImageSnafu { path })?
        .resize(640, 640, FilterType::Lanczos3)
        .into_rgba8();
    let (width, height) = decoded.dimensions();
    Image::from_rgba(width, height, decoded.into_raw()).context(ValidateImageSnafu)
}
