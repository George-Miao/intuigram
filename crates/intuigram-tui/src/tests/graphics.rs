use std::ffi::OsString;

use rasterm::{CellSize, Environment, Image, Multiplexer};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::{TerminalOptions, Viewport};

use super::*;
use crate::source::graphics::{GraphicsProtocol, GraphicsRequest, GraphicsState};
use crate::source::terminal::draw_terminal_view;

#[test]
fn ghostty_selects_kitty_unicode_graphics() {
    assert_eq!(
        protocol("xterm-ghostty", None),
        GraphicsProtocol::KittyUnicode
    );
    assert_eq!(protocol("xterm-256color", None), GraphicsProtocol::Text);
}

#[test]
fn ghostty_inside_zellij_keeps_kitty_unicode_graphics() {
    assert_eq!(
        protocol("xterm-256color", Some("ghostty")),
        GraphicsProtocol::KittyUnicode
    );
}

#[test]
fn ghostty_upload_creates_only_a_virtual_unicode_placement() {
    let mut state = GraphicsState::new(GraphicsProtocol::KittyUnicode);
    let mut output = Vec::new();
    state
        .sync(&mut output, &[graphics_request()])
        .expect("memory output should accept a graphics request");
    let encoded = String::from_utf8(output).expect("graphics commands are ASCII");

    assert!(encoded.starts_with("\u{1b}_Gq=2,a=T,U=1,f=32,s=1,v=1,i=42,c=32,r=6,m=0;"));
    assert!(!encoded.contains("\u{1b}[10;8H"));
    assert!(encoded.contains("/wAA/w=="));
    assert!(encoded.ends_with("\u{1b}\\"));
}

#[test]
fn portrait_images_reserve_only_their_aspect_fitted_cells() {
    let mut current = image_message_view();
    current.media_previews = vec![intuigram_app::MediaPreviewView {
        chat: ChatId(10),
        message: MessageId(40),
        image: intuigram_app::InlineImage::from_rgba(48, 96, vec![255; 48 * 96 * 4])
            .expect("fixture pixels should match their dimensions"),
    }];

    let (_, graphics) =
        render_test_frame_with_graphics(&current, 100, 40, GraphicsProtocol::KittyUnicode);
    assert_eq!(graphics.requests()[0].size.columns, 6);
    assert_eq!(graphics.requests()[0].size.rows, 6);
}

#[test]
fn kitty_render_uses_unicode_placeholders_without_redundant_media_metadata() {
    let mut current = image_message_view();
    current.media_previews = vec![intuigram_app::MediaPreviewView {
        chat: ChatId(10),
        message: MessageId(40),
        image: intuigram_app::InlineImage::from_rgba(1, 1, vec![255, 0, 0, 255])
            .expect("fixture pixels should match their dimensions"),
    }];

    let (frame, graphics) =
        render_test_frame_with_graphics(&current, 100, 40, GraphicsProtocol::KittyUnicode);
    let text = symbols(&frame.buffer);

    assert!(text.contains('\u{10eeee}'));
    assert!(!text.contains('▀'));
    assert!(text.contains("caption"));
    assert!(!text.contains("1280 × 720"));
    assert!(!text.contains("2 MB"));
    assert_eq!(graphics.requests().len(), 1);
    assert_eq!(graphics.requests()[0].size.columns, 12);
    assert_eq!(graphics.requests()[0].size.rows, 6);
    assert!(graphics.requests()[0].x > 0);
    assert!(graphics.requests()[0].y > 0);
}

#[test]
fn kitty_upload_precedes_its_unicode_placeholder() {
    let mut current = image_message_view();
    current.media_previews = vec![intuigram_app::MediaPreviewView {
        chat: ChatId(10),
        message: MessageId(40),
        image: intuigram_app::InlineImage::from_rgba(1, 1, vec![255, 0, 0, 255])
            .expect("fixture pixels should match their dimensions"),
    }];
    let mut output = Vec::new();
    {
        let backend = CrosstermBackend::new(&mut output);
        let options = TerminalOptions {
            viewport: Viewport::Fixed(Rect::new(0, 0, 100, 40)),
        };
        let mut terminal = Terminal::with_options(backend, options)
            .expect("fixed memory terminal should initialize");
        let mut graphics = GraphicsState::new(GraphicsProtocol::KittyUnicode);

        draw_terminal_view(
            &mut terminal,
            &mut graphics,
            &EffectiveKeymap::defaults(),
            ViewMode::Default,
            GraphicsProtocol::KittyUnicode,
            Multiplexer::None,
            &current,
        )
        .expect("memory terminal should render an image");
    }

    let upload = byte_offset(&output, b"\x1b_Gq=2,a=T");
    let placeholder = byte_offset(&output, "\u{10eeee}".as_bytes());
    assert!(upload < placeholder);
}

#[test]
fn graphics_state_reuses_unchanged_uploads_and_deletes_stale_images() {
    let request = graphics_request();
    let mut state = GraphicsState::new(GraphicsProtocol::KittyUnicode);
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
    let mut state = GraphicsState::new(GraphicsProtocol::KittyUnicode);
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
        id: 42,
        image: Image::from_rgba(1, 1, vec![255, 0, 0, 255])
            .expect("fixture pixels should match their dimensions")
            .into(),
        size: CellSize {
            columns: 32,
            rows: 6,
        },
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

fn image_message_view() -> View {
    let mut current = view(Vec::new());
    current.chats = vec![ChatView {
        id: ChatId(10),
        title: "Intuigram".to_owned(),
        preview: "photo".to_owned(),
        status: "connected".to_owned(),
        unread: 0,
        pinned: false,
        can_pin_messages: true,
        kind: ChatKind::Private,
        folders: vec![0],
    }];
    current.active_chat = Some(0);
    current.focus = Focus::Composer;
    current.messages = vec![MessageView {
        id: MessageId(40),
        sender: "Ada".to_owned(),
        body: "caption".to_owned(),
        timestamp: "now".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: None,
        details: MessageDetails {
            media: Some(MediaCard {
                kind: MediaKind::Photo,
                title: "Photo".to_owned(),
                description: "image".to_owned(),
                details: vec!["1280 × 720".to_owned(), "2 MB".to_owned()],
                poll: None,
                remote_id: Some("42".to_owned()),
            }),
            ..MessageDetails::default()
        },
    }];
    current
}

fn symbols(buffer: &ratatui::buffer::Buffer) -> String {
    buffer.content.iter().map(|cell| cell.symbol()).collect()
}

fn byte_offset(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
        .expect("terminal output should contain the requested sequence")
}
