use std::ffi::OsStr;

use super::*;
use crate::source::graphics::{GraphicsProtocol, GraphicsRequest, GraphicsState, encode_upload};

#[test]
fn ghostty_selects_kitty_unicode_graphics() {
    assert_eq!(
        GraphicsProtocol::detect(Some(OsStr::new("xterm-ghostty"))),
        GraphicsProtocol::KittyUnicode
    );
    assert_eq!(
        GraphicsProtocol::detect(Some(OsStr::new("xterm-256color"))),
        GraphicsProtocol::Text
    );
}

#[test]
fn kitty_upload_is_quiet_rgba_with_a_virtual_placement() {
    let image = intuigram_app::InlineImage::from_rgba(1, 1, vec![255, 0, 0, 255])
        .expect("fixture pixels should match their dimensions");

    let encoded =
        String::from_utf8(encode_upload(42, &image, 32, 6)).expect("graphics commands are ASCII");

    assert!(encoded.starts_with("\u{1b}_Ga=T,f=32,s=1,v=1,i=42,U=1,c=32,r=6,q=2,m=0;"));
    assert!(encoded.contains("/wAA/w=="));
    assert!(encoded.ends_with("\u{1b}\\"));
}

#[test]
fn kitty_render_uses_unicode_placeholders_and_retains_text_fallback_metadata() {
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
    assert_eq!(graphics.requests().len(), 1);
    assert_eq!(graphics.requests()[0].columns, 32);
    assert_eq!(graphics.requests()[0].rows, 6);
}

#[test]
fn graphics_state_reuses_unchanged_uploads_and_deletes_stale_images() {
    let request = graphics_request();
    let mut state = GraphicsState::default();
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
    let mut state = GraphicsState::default();
    let mut output = Vec::new();

    state
        .sync(&mut output, std::slice::from_ref(&request))
        .expect("memory output should accept a graphics request");
    let first_upload = output.len();
    request.rows = 5;
    state
        .sync(&mut output, std::slice::from_ref(&request))
        .expect("memory output should accept a resized placement");

    assert!(output.len() > first_upload);
    assert!(output[first_upload..].starts_with(b"\x1b_Ga=T"));
}

fn graphics_request() -> GraphicsRequest {
    GraphicsRequest {
        id: 42,
        image: intuigram_app::InlineImage::from_rgba(1, 1, vec![255, 0, 0, 255])
            .expect("fixture pixels should match their dimensions"),
        columns: 32,
        rows: 6,
    }
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
                details: Vec::new(),
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
