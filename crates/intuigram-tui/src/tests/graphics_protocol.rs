use std::ffi::OsString;

use rasterm::{CellPixels, CellSize, Environment, Image, ImageId, Multiplexer};

use crate::source::graphics::{GraphicsProtocol, GraphicsRequest};

#[test]
fn ghostty_selects_verified_cursor_anchored_kitty_graphics() {
    assert_eq!(
        protocol("xterm-ghostty", None),
        GraphicsProtocol::KittyLegacy
    );
    assert_eq!(protocol("xterm-256color", None), GraphicsProtocol::Text);
}

#[test]
fn zellij_on_ghostty_uses_cursor_anchored_kitty_graphics() {
    assert_eq!(
        protocol("xterm-256color", Some("ghostty")),
        GraphicsProtocol::KittyLegacy
    );
}

#[test]
fn zellij_without_a_kitty_host_keeps_sixel() {
    assert_eq!(
        protocol("xterm-256color", Some("Apple_Terminal")),
        GraphicsProtocol::Sixel
    );
}

#[test]
fn kitty_unicode_upload_creates_only_a_virtual_placement() {
    let mut state = rasterm::Renderer::new(GraphicsProtocol::KittyUnicode);
    let mut output = Vec::new();
    state
        .sync(&mut output, &[graphics_request()])
        .expect("memory output should accept a graphics request");
    let encoded = String::from_utf8(output).expect("graphics commands are ASCII");

    assert!(encoded.starts_with("\u{1b}_Gq=2,a=T,C=1,U=1,z=-1,f=32,s=1,v=1,i=42,c=32,r=6,m=0;"));
    assert!(!encoded.contains("\u{1b}[10;8H"));
    assert!(encoded.contains("/wAA/w=="));
    assert!(encoded.ends_with("\u{1b}\\"));
}

#[test]
fn graphics_state_reuses_unchanged_uploads_and_deletes_stale_images() {
    let request = graphics_request();
    let mut state = rasterm::Renderer::new(GraphicsProtocol::KittyUnicode);
    let mut output = Vec::new();

    state
        .sync(&mut output, std::slice::from_ref(&request))
        .expect("memory output should accept a graphics request");
    let uploaded_bytes = output.len();
    state
        .sync(&mut output, std::slice::from_ref(&request))
        .expect("memory output should accept an unchanged frame");
    assert_eq!(output.len(), uploaded_bytes);

    state
        .sync(&mut output, &[])
        .expect("memory output should accept a deletion");
    assert!(output[uploaded_bytes..].ends_with(b"\x1b_Ga=d,d=I,i=42,q=2\x1b\\"));
}

#[test]
fn graphics_state_reuploads_a_resized_placement() {
    let mut request = graphics_request();
    let mut state = rasterm::Renderer::new(GraphicsProtocol::KittyUnicode);
    let mut output = Vec::new();

    state
        .sync(&mut output, std::slice::from_ref(&request))
        .expect("memory output should accept a graphics request");
    let first_upload = output.len();
    request.size.rows = 5;
    state
        .sync(&mut output, std::slice::from_ref(&request))
        .expect("memory output should accept a resized placement");

    assert!(output.len() > first_upload);
    assert!(output[first_upload..].starts_with(b"\x1b_Ga=d,d=I,i=42,q=2"));
    assert!(
        output[first_upload..]
            .windows(b"\x1b_Gq=2,a=T".len())
            .any(|window| window == b"\x1b_Gq=2,a=T")
    );
}

fn graphics_request() -> GraphicsRequest {
    GraphicsRequest {
        id: ImageId::new(42).expect("fixture ID is nonzero"),
        image: Image::from_rgba(1, 1, vec![255, 0, 0, 255])
            .expect("fixture pixels should match their dimensions")
            .into(),
        size: CellSize {
            columns: 32,
            rows: 6,
        },
        cell_pixels: CellPixels::default(),
        x: 7,
        y: 9,
        multiplexer: Multiplexer::None,
    }
}

fn protocol(term: &str, term_program: Option<&str>) -> GraphicsProtocol {
    Environment {
        term: Some(OsString::from(term)),
        term_program: term_program.map(OsString::from),
        multiplexer: term_program.map_or(Multiplexer::None, |_| Multiplexer::Zellij),
        ..Environment::default()
    }
    .protocol()
}
