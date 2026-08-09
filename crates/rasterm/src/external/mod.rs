mod chafa;
mod driver;
mod ueberzug;

use std::io::Cursor;

pub use chafa::ChafaCommand;
pub(crate) use driver::Driver;
use image::{ExtendedColorType, ImageEncoder};
use snafu::{ResultExt, Snafu};
pub use ueberzug::UeberzugCommand;

use crate::Image;

/// External terminal raster adapter failure.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    /// A PNG payload could not be encoded for an external renderer.
    #[snafu(display("failed to encode an external-renderer PNG payload"))]
    Encode { source: image::ImageError },

    /// An external renderer process could not be started.
    #[snafu(display("failed to start external renderer `{program}`"))]
    Spawn {
        /// Executable name.
        program: &'static str,

        /// Process creation failure.
        source: std::io::Error,
    },

    /// An external renderer pipe was unavailable.
    #[snafu(display("external renderer `{program}` did not expose its {pipe} pipe"))]
    MissingPipe {
        /// Executable name.
        program: &'static str,

        /// Missing standard pipe.
        pipe: &'static str,
    },

    /// Image bytes or a layer command could not be sent to a renderer.
    #[snafu(display("failed to write to external renderer `{program}`"))]
    Write {
        /// Executable name.
        program: &'static str,

        /// Pipe write failure.
        source: std::io::Error,
    },

    /// A one-shot external renderer could not be joined.
    #[snafu(display("failed to wait for external renderer `{program}`"))]
    Wait {
        /// Executable name.
        program: &'static str,

        /// Process wait failure.
        source: std::io::Error,
    },

    /// An external renderer rejected an otherwise valid request.
    #[snafu(display("external renderer `{program}` exited unsuccessfully: {detail}"))]
    Rejected {
        /// Executable name.
        program: &'static str,

        /// Sanitized diagnostic output.
        detail: String,
    },

    /// A private overlay image could not be written.
    #[snafu(display("failed to write external-renderer image `{}`", path.display()))]
    WriteImage {
        /// Private temporary image path.
        path: std::path::PathBuf,

        /// File write failure.
        source: std::io::Error,
    },
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) fn png(image: &Image) -> Result<Vec<u8>> {
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(Cursor::new(&mut png))
        .write_image(
            image.rgba(),
            image.width(),
            image.height(),
            ExtendedColorType::Rgba8,
        )
        .context(EncodeSnafu)?;
    Ok(png)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{ChafaCommand, UeberzugCommand};
    use crate::{CellSize, ImageId};

    const SIZE: CellSize = CellSize {
        columns: 12,
        rows: 6,
    };

    #[test]
    fn external_renderers_receive_shell_free_requests() {
        let chafa = ChafaCommand::symbols(Path::new("/tmp/a b.png"), SIZE);
        assert_eq!(chafa.program, "chafa");
        assert_eq!(
            chafa.arguments.last().map(String::as_str),
            Some("/tmp/a b.png")
        );

        let command = UeberzugCommand::add(
            ImageId::new(42).expect("fixture ID is nonzero"),
            Path::new("/tmp/a b.png"),
            7,
            9,
            SIZE,
        );
        assert_eq!(
            command.json_line(),
            "{\"action\":\"add\",\"identifier\":\"rasterm-42\",\"path\":\"/tmp/a \
             b.png\",\"x\":7,\"y\":9,\"max_width\":12,\"max_height\":6}\n"
        );
    }
}
