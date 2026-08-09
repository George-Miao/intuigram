mod encode;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::sync::Arc;

use snafu::{ResultExt, Snafu};

use crate::external::Driver;
use crate::{CellPixels, CellSize, Image, ImageId, Multiplexer, Protocol};

/// One terminal-native image placement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Placement {
    /// Stable nonzero image ID.
    pub id: ImageId,

    /// Image pixels.
    pub image: Arc<Image>,

    /// Occupied terminal cells.
    pub size: CellSize,

    /// Pixel geometry used by protocols that cannot size in terminal cells.
    pub cell_pixels: CellPixels,

    /// Zero-based terminal column.
    pub x: u16,

    /// Zero-based terminal row.
    pub y: u16,

    /// Multiplexer transport behavior.
    pub multiplexer: Multiplexer,
}

/// Terminal graphics output failure.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    /// A graphics sequence could not be written.
    #[snafu(display("failed to write terminal graphics output"))]
    Write { source: std::io::Error },

    /// A protocol-specific image payload could not be encoded.
    #[snafu(display("failed to encode terminal image payload"))]
    Encode { source: image::ImageError },

    /// An external graphics adapter failed.
    #[snafu(display("external terminal graphics adapter failed"))]
    External { source: crate::external::Error },
}

/// Result returned by terminal graphics lifecycle operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Stateful native-image lifecycle for repeated terminal frames.
#[derive(Debug)]
pub struct Renderer {
    protocol: Protocol,
    images: HashMap<ImageId, ImageState>,
    external: Driver,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ImageState {
    fingerprint: u64,
    multiplexer: Multiplexer,
}

impl Renderer {
    /// Starts an empty lifecycle for one selected protocol.
    #[must_use]
    pub fn new(protocol: Protocol) -> Self {
        Self {
            protocol,
            images: HashMap::new(),
            external: Driver::new(protocol),
        }
    }

    /// Selected protocol.
    #[must_use]
    pub const fn protocol(&self) -> Protocol {
        self.protocol
    }

    /// Deletes stale images and transmits changed placements.
    pub fn sync(&mut self, writer: &mut impl Write, placements: &[Placement]) -> Result<()> {
        let visible = placements
            .iter()
            .map(|placement| placement.id)
            .collect::<HashSet<_>>();
        let stale = self
            .images
            .keys()
            .copied()
            .filter(|id| !visible.contains(id))
            .collect::<Vec<_>>();
        for id in stale {
            let multiplexer = self.images[&id].multiplexer;
            self.delete(writer, id, multiplexer)?;
        }
        for placement in placements {
            let fingerprint = fingerprint(self.protocol, placement);
            if self
                .images
                .get(&placement.id)
                .is_some_and(|state| state.fingerprint == fingerprint)
            {
                continue;
            }
            if self.images.contains_key(&placement.id) {
                self.delete(writer, placement.id, placement.multiplexer)?;
            }
            let encoded = match self.protocol {
                Protocol::Ueberzug | Protocol::Chafa => {
                    self.external.place(placement).context(ExternalSnafu)?
                }
                Protocol::Text
                | Protocol::KittyUnicode
                | Protocol::KittyLegacy
                | Protocol::Iterm2
                | Protocol::Sixel => encode::placement(self.protocol, placement)?,
            };
            writer.write_all(&encoded).context(WriteSnafu)?;
            self.images.insert(
                placement.id,
                ImageState {
                    fingerprint,
                    multiplexer: placement.multiplexer,
                },
            );
        }
        writer.flush().context(WriteSnafu)
    }

    /// Deletes every retained terminal image.
    pub fn clear(&mut self, writer: &mut impl Write) -> Result<()> {
        for (id, multiplexer) in self
            .images
            .iter()
            .map(|(&id, state)| (id, state.multiplexer))
            .collect::<Vec<_>>()
        {
            self.delete(writer, id, multiplexer)?;
        }
        writer.flush().context(WriteSnafu)
    }

    fn delete(&mut self, writer: &mut impl Write, id: ImageId, mux: Multiplexer) -> Result<()> {
        self.external.delete(id).context(ExternalSnafu)?;
        writer
            .write_all(&encode::delete(self.protocol, id, mux))
            .context(WriteSnafu)?;
        self.images.remove(&id);
        Ok(())
    }
}

fn fingerprint(protocol: Protocol, placement: &Placement) -> u64 {
    let mut hash = 14_695_981_039_346_656_037_u64;
    let origin = if protocol.uses_unicode_placeholders() {
        (0, 0)
    } else {
        (placement.x, placement.y)
    };
    for byte in placement
        .image
        .width()
        .to_le_bytes()
        .into_iter()
        .chain(placement.image.height().to_le_bytes())
        .chain(origin.0.to_le_bytes())
        .chain(origin.1.to_le_bytes())
        .chain(placement.size.columns.to_le_bytes())
        .chain(placement.size.rows.to_le_bytes())
        .chain(placement.cell_pixels.width.to_le_bytes())
        .chain(placement.cell_pixels.height.to_le_bytes())
        .chain(placement.image.rgba().iter().copied())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(1_099_511_628_211);
    }
    hash
}
