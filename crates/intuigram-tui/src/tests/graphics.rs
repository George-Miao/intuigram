use std::ffi::OsString;

use rasterm::{CellPixels, CellSize, Environment, Image, ImageId, Multiplexer};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::{TerminalOptions, Viewport};

use super::*;
use crate::source::graphics::{GraphicsProtocol, GraphicsRequest};
use crate::source::terminal::{TerminalFrameState, draw_terminal_view};

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
fn portrait_images_reserve_only_their_aspect_fitted_cells() {
    let mut current = image_message_view();
    current.media_previews = vec![intuigram_lib::MediaPreviewView {
        chat: ChatId(10),
        message: MessageId(40),
        image: intuigram_lib::InlineImage::from_rgba(48, 96, vec![255; 48 * 96 * 4])
            .expect("fixture pixels should match their dimensions"),
    }];

    let (_, graphics) =
        render_test_frame_with_graphics(&current, 100, 40, GraphicsProtocol::KittyUnicode);
    assert_eq!(graphics.requests()[0].size.columns, 12);
    assert_eq!(graphics.requests()[0].size.rows, 12);
}

#[test]
fn kitty_render_uses_unicode_placeholders_without_redundant_media_metadata() {
    let mut current = image_message_view();
    current.media_previews = vec![intuigram_lib::MediaPreviewView {
        chat: ChatId(10),
        message: MessageId(40),
        image: intuigram_lib::InlineImage::from_rgba(1, 1, vec![255, 0, 0, 255])
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
    assert_eq!(graphics.requests()[0].size.columns, 24);
    assert_eq!(graphics.requests()[0].size.rows, 12);
    assert!(graphics.requests()[0].x > 0);
    assert!(graphics.requests()[0].y > 0);
}

#[test]
fn inline_image_geometry_shrinks_inside_a_narrow_transcript() {
    let mut current = image_message_view();
    current.media_previews = vec![intuigram_lib::MediaPreviewView {
        chat: ChatId(10),
        message: MessageId(40),
        image: intuigram_lib::InlineImage::from_rgba(1, 1, vec![255, 0, 0, 255])
            .expect("fixture pixels should match their dimensions"),
    }];

    let (_, graphics) =
        render_test_frame_with_graphics(&current, 24, 40, GraphicsProtocol::KittyUnicode);

    assert!(graphics.requests()[0].size.columns <= 20);
    assert!(graphics.requests()[0].size.rows <= 12);
}

#[test]
fn completed_image_does_not_leave_removed_message_cells() {
    let mut current = image_message_view();
    current.media_previews = vec![intuigram_lib::MediaPreviewView {
        chat: ChatId(10),
        message: MessageId(40),
        image: intuigram_lib::InlineImage::from_rgba(1, 1, vec![255, 0, 0, 255])
            .expect("fixture pixels should match their dimensions"),
    }];
    current.messages[0].body = "moved message ".repeat(12);
    let mut output = Vec::new();
    {
        let backend = CrosstermBackend::new(&mut output);
        let options = TerminalOptions {
            viewport: Viewport::Fixed(Rect::new(0, 0, 160, 40)),
        };
        let mut terminal = Terminal::with_options(backend, options)
            .expect("fixed memory terminal should initialize");
        let mut frame_state =
            TerminalFrameState::new(GraphicsProtocol::KittyUnicode, Multiplexer::None)
                .expect("graphics worker should start");

        draw_terminal_view(
            &mut terminal,
            &mut frame_state,
            &EffectiveKeymap::defaults(),
            ViewOptions::default(),
            &current,
        )
        .expect("initial frame should render");
        wait_for_graphics(&mut frame_state);

        current.active_thread = Some(MessageId(40));
        draw_terminal_view(
            &mut terminal,
            &mut frame_state,
            &EffectiveKeymap::defaults(),
            ViewOptions::default(),
            &current,
        )
        .expect("thread layout should render");
    }

    let (expected, _) =
        render_test_frame_with_graphics(&current, 160, 40, GraphicsProtocol::KittyUnicode);
    let mut parser = vt100::Parser::new(40, 160, 0);
    parser.process(&output);
    assert_terminal_symbols(parser.screen(), &expected.buffer);
}

#[test]
fn background_kitty_upload_precedes_the_followup_placeholder() {
    let mut current = image_message_view();
    current.media_previews = vec![intuigram_lib::MediaPreviewView {
        chat: ChatId(10),
        message: MessageId(40),
        image: intuigram_lib::InlineImage::from_rgba(1, 1, vec![255, 0, 0, 255])
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
        let mut frame_state =
            TerminalFrameState::new(GraphicsProtocol::KittyUnicode, Multiplexer::None)
                .expect("graphics worker should start");

        draw_terminal_view(
            &mut terminal,
            &mut frame_state,
            &EffectiveKeymap::defaults(),
            ViewOptions::default(),
            &current,
        )
        .expect("memory terminal should render an image");
        wait_for_graphics(&mut frame_state);
        draw_terminal_view(
            &mut terminal,
            &mut frame_state,
            &EffectiveKeymap::defaults(),
            ViewOptions::default(),
            &current,
        )
        .expect("prepared graphics should render on the followup frame");
    }

    let first_placeholder = byte_offset(&output, "\u{10eeee}".as_bytes());
    let upload = byte_offset(&output, b"\x1b_Gq=2,a=T");
    let followup_placeholder = output
        .windows("\u{10eeee}".len())
        .rposition(|window| window == "\u{10eeee}".as_bytes())
        .expect("followup output should contain a Unicode placeholder");
    assert!(first_placeholder < upload);
    assert!(upload < followup_placeholder);
    assert_eq!(
        output
            .windows(b"caption".len())
            .filter(|window| *window == b"caption")
            .count(),
        1,
        "graphics completion should not repaint unchanged text"
    );
}

#[test]
fn attachment_preview_preserves_composer_cursor() {
    let mut current = image_message_view();
    current.messages.clear();
    current.composer.attachments = vec![intuigram_lib::AttachmentView {
        id: intuigram_lib::AttachmentId(7),
        kind: intuigram_lib::AttachmentKind::Photo,
        name: "clipboard.png".to_owned(),
        preview: Some(
            intuigram_lib::InlineImage::from_rgba(1, 1, vec![255, 0, 0, 255])
                .expect("fixture pixels should match their dimensions"),
        ),
        active: true,
    }];

    let mut expected_terminal =
        Terminal::new(TestBackend::new(100, 28)).expect("test terminal should initialize");
    expected_terminal
        .draw(|frame| render(frame, &current, &EffectiveKeymap::defaults()))
        .expect("Composer should render");
    let expected = expected_terminal.backend().cursor_position();

    let mut output = Vec::new();
    {
        let backend = CrosstermBackend::new(&mut output);
        let options = TerminalOptions {
            viewport: Viewport::Fixed(Rect::new(0, 0, 100, 28)),
        };
        let mut terminal = Terminal::with_options(backend, options)
            .expect("fixed memory terminal should initialize");
        let mut frame_state =
            TerminalFrameState::new(GraphicsProtocol::KittyLegacy, Multiplexer::None)
                .expect("graphics worker should start");

        draw_terminal_view(
            &mut terminal,
            &mut frame_state,
            &EffectiveKeymap::defaults(),
            ViewOptions::default(),
            &current,
        )
        .expect("initial attachment frame should render");
        wait_for_graphics(&mut frame_state);
        draw_terminal_view(
            &mut terminal,
            &mut frame_state,
            &EffectiveKeymap::defaults(),
            ViewOptions::default(),
            &current,
        )
        .expect("prepared attachment preview should render");
    }

    let mut parser = vt100::Parser::new(28, 100, 0);
    parser.process(&output);
    assert_eq!(
        parser.screen().cursor_position(),
        (expected.y, expected.x),
        "native attachment output must restore the Composer cursor"
    );
}

fn wait_for_graphics(state: &mut TerminalFrameState) {
    for _ in 0..100 {
        let waker = futures_util::task::noop_waker();
        let mut cx = std::task::Context::from_waker(&waker);
        if state.poll_redraw(&mut cx).is_ready() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("graphics worker did not complete the tiny fixture in time");
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

fn image_message_view() -> View {
    let mut current = view(Vec::new());
    current.chats = vec![ChatView {
        id: ChatId(10),
        title: "Intuigram".to_owned(),
        preview: "photo".to_owned(),
        preview_sender: None,
        preview_sender_peer: None,
        preview_timestamp: String::new(),
        status: "connected".to_owned(),
        unread: 0,
        pinned: false,
        can_pin_messages: true,
        has_topics: false,
        has_direct_messages: false,
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
            sender_peer: None,
            media: Some(MediaCard {
                kind: MediaKind::Photo,
                title: "Photo".to_owned(),
                description: "image".to_owned(),
                details: vec!["1280 × 720".to_owned(), "2 MB".to_owned()],
                poll: None,
                specialized: None,
                remote_id: Some("42".to_owned()),
            }),
            ..MessageDetails::default()
        },
    }];
    current
}

fn assert_terminal_symbols(screen: &vt100::Screen, expected: &ratatui::buffer::Buffer) {
    for y in 0..expected.area.height {
        for x in 0..expected.area.width {
            let expected = expected[(x, y)].symbol();
            let expected = if expected == " " { "" } else { expected };
            let actual = screen
                .cell(y, x)
                .expect("VT screen should contain every expected cell")
                .contents();
            let actual = if actual == " " { "" } else { actual };
            assert_eq!(actual, expected, "terminal cell ({x}, {y}) differs");
        }
    }
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
