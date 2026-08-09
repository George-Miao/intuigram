use std::fmt::Write as _;

use base64::Engine as _;

use crate::{ImageId, Multiplexer, Placement};

const RAW_CHUNK_BYTES: usize = 3_072;

pub(crate) fn encode_unicode(placement: &Placement) -> Vec<u8> {
    wrap(encode_transmission(placement, true), placement.multiplexer)
}

pub(crate) fn encode_legacy(placement: &Placement) -> Vec<u8> {
    let mut output = format!(
        "\x1b[{};{}H",
        placement.y.saturating_add(1),
        placement.x.saturating_add(1)
    )
    .into_bytes();
    output.extend_from_slice(&encode_transmission(placement, false));
    wrap(output, placement.multiplexer)
}

pub(crate) fn delete(id: ImageId, multiplexer: Multiplexer) -> Vec<u8> {
    wrap(
        format!("\x1b_Ga=d,d=I,i={},q=2\x1b\\", id.get()).into_bytes(),
        multiplexer,
    )
}

pub(crate) fn wrap(bytes: Vec<u8>, multiplexer: Multiplexer) -> Vec<u8> {
    if multiplexer != Multiplexer::Tmux {
        return bytes;
    }
    let mut wrapped = b"\x1bPtmux;".to_vec();
    for byte in bytes {
        if byte == 0x1b {
            wrapped.push(0x1b);
        }
        wrapped.push(byte);
    }
    wrapped.extend_from_slice(b"\x1b\\");
    wrapped
}

fn encode_transmission(placement: &Placement, unicode: bool) -> Vec<u8> {
    let chunks = placement
        .image
        .rgba()
        .chunks(RAW_CHUNK_BYTES)
        .collect::<Vec<_>>();
    let mut output = Vec::new();
    for (index, chunk) in chunks.iter().enumerate() {
        let more = usize::from(index + 1 < chunks.len());
        let mut command = String::new();
        if index == 0 {
            write_first(&mut command, placement, unicode, more);
        } else {
            write!(command, "\x1b_Gm={more};")
                .expect("writing a graphics command to memory cannot fail");
        }
        output.extend_from_slice(command.as_bytes());
        output.extend_from_slice(
            base64::engine::general_purpose::STANDARD
                .encode(chunk)
                .as_bytes(),
        );
        output.extend_from_slice(b"\x1b\\");
    }
    output
}

fn write_first(output: &mut String, placement: &Placement, unicode: bool, more: usize) {
    let virtual_placement = if unicode { ",U=1" } else { ",C=1" };
    write!(
        output,
        "\x1b_Gq=2,a=T{virtual_placement},f=32,s={},v={},i={},c={},r={},m={more};",
        placement.image.width(),
        placement.image.height(),
        placement.id.get(),
        placement.size.columns,
        placement.size.rows,
    )
    .expect("writing a graphics command to memory cannot fail");
}
