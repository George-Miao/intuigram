use std::sync::Arc;

use super::{Placement, Renderer};
use crate::{CellPixels, CellSize, Image, ImageId, Multiplexer, Protocol};

#[test]
fn kitty_unicode_upload_is_virtual_and_position_independent() {
    let mut renderer = Renderer::new(Protocol::KittyUnicode);
    let mut placement = placement(Multiplexer::None);
    let mut output = Vec::new();

    renderer
        .sync(&mut output, std::slice::from_ref(&placement))
        .expect("memory output should accept a Kitty upload");
    let first = output.len();
    let text = String::from_utf8(output.clone()).expect("Kitty commands are ASCII");
    assert!(text.starts_with("\x1b_Gq=2,a=T,C=1,U=1,f=32,s=1,v=1,i=42,c=12,r=6,m=0;"));
    assert!(!text.contains("\x1b[10;8H"));

    placement.x = 20;
    placement.y = 30;
    renderer
        .sync(&mut output, &[placement])
        .expect("moving a virtual placement should not fail");
    assert_eq!(output.len(), first);
}

#[test]
fn kitty_legacy_is_cursor_anchored_and_tmux_escaped() {
    let mut renderer = Renderer::new(Protocol::KittyLegacy);
    let mut output = Vec::new();
    renderer
        .sync(&mut output, &[placement(Multiplexer::Tmux)])
        .expect("memory output should accept a wrapped Kitty upload");

    assert!(output.starts_with(b"\x1bPtmux;\x1b\x1b[10;8H\x1b\x1b_G"));
    assert!(output.ends_with(b"\x1b\\"));
}

#[test]
fn iterm2_and_sixel_encoders_emit_their_native_framing() {
    let request = placement(Multiplexer::None);
    let mut iterm = Renderer::new(Protocol::Iterm2);
    let mut iterm_output = Vec::new();
    iterm
        .sync(&mut iterm_output, std::slice::from_ref(&request))
        .expect("the PNG fixture should encode for iTerm2");
    assert!(
        iterm_output
            .windows(b"\x1b]1337;File=inline=1".len())
            .any(|window| window == b"\x1b]1337;File=inline=1")
    );

    let mut sixel = Renderer::new(Protocol::Sixel);
    let mut sixel_output = Vec::new();
    sixel
        .sync(&mut sixel_output, &[request])
        .expect("the RGBA fixture should encode as Sixel");
    assert!(sixel_output.starts_with(b"\x1b[10;8H\x1bPq\"1;1;96;96"));
    assert!(sixel_output.ends_with(b"\x1b\\"));
}

#[test]
fn stale_kitty_images_are_deleted() {
    let mut renderer = Renderer::new(Protocol::KittyUnicode);
    let mut output = Vec::new();
    renderer
        .sync(&mut output, &[placement(Multiplexer::None)])
        .expect("memory output should accept a Kitty upload");
    let uploaded = output.len();
    renderer
        .sync(&mut output, &[])
        .expect("memory output should accept a Kitty deletion");
    assert_eq!(&output[uploaded..], b"\x1b_Ga=d,d=I,i=42,q=2\x1b\\");
}

fn placement(multiplexer: Multiplexer) -> Placement {
    Placement {
        id: ImageId::new(42).expect("fixture ID is nonzero"),
        image: Arc::new(
            Image::from_rgba(1, 1, vec![255, 0, 0, 255])
                .expect("fixture dimensions should match its pixels"),
        ),
        size: CellSize {
            columns: 12,
            rows: 6,
        },
        cell_pixels: CellPixels {
            width: 8,
            height: 16,
        },
        x: 7,
        y: 9,
        multiplexer,
    }
}
