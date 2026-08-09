//! Terminal-independent raster image protocol selection and lifecycle.
//!
//! `rasterm` owns terminal detection and byte encoding. It deliberately knows
//! nothing about terminal UI frameworks or application view models.

mod environment;
mod external;
mod geometry;
mod id;
mod image;
mod kitty;
mod placeholder;
mod protocol;
mod renderer;
mod sixel;
mod text;

pub use environment::{Environment, Multiplexer};
pub use external::{ChafaCommand, UeberzugCommand};
pub use geometry::{CellBounds, CellPixels, CellSize, fit_cells};
pub use id::ImageId;
pub use image::{Error as ImageError, Image, Result as ImageResult};
pub use placeholder::unicode_placeholder;
pub use protocol::Protocol;
pub use renderer::{Error, Placement, Renderer, Result};
pub use text::{TextCell, text_cells};

/// Kitty's Unicode image-placeholder base code point.
pub const PLACEHOLDER: char = '\u{10eeee}';
