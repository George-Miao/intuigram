use std::io::Cursor;

use base64::Engine as _;
use image::{ExtendedColorType, ImageEncoder};
use snafu::ResultExt;

use super::{EncodeSnafu, Placement, Result};
use crate::{Multiplexer, Protocol, kitty, sixel};

pub(super) fn placement(protocol: Protocol, placement: &Placement) -> Result<Vec<u8>> {
    match protocol {
        Protocol::KittyUnicode => Ok(kitty::encode_unicode(placement)),
        Protocol::KittyLegacy => Ok(kitty::encode_legacy(placement)),
        Protocol::Iterm2 => encode_iterm2(placement),
        Protocol::Sixel => Ok(sixel::encode(placement)),
        Protocol::Text | Protocol::Ueberzug | Protocol::Chafa => Ok(Vec::new()),
    }
}

pub(super) fn delete(protocol: Protocol, id: u32, mux: Multiplexer) -> Vec<u8> {
    match protocol {
        Protocol::KittyUnicode | Protocol::KittyLegacy => kitty::delete(id, mux),
        Protocol::Iterm2
        | Protocol::Sixel
        | Protocol::Text
        | Protocol::Ueberzug
        | Protocol::Chafa => Vec::new(),
    }
}

fn encode_iterm2(placement: &Placement) -> Result<Vec<u8>> {
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(Cursor::new(&mut png))
        .write_image(
            placement.image.rgba(),
            placement.image.width(),
            placement.image.height(),
            ExtendedColorType::Rgba8,
        )
        .context(EncodeSnafu)?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(png);
    Ok(kitty::wrap(
        format!(
            "\x1b[{};{}H\x1b]1337;File=inline=1;width={};height={};preserveAspectRatio=1:\
             {encoded}\x07",
            placement.y.saturating_add(1),
            placement.x.saturating_add(1),
            placement.size.columns,
            placement.size.rows,
        )
        .into_bytes(),
        placement.multiplexer,
    ))
}
