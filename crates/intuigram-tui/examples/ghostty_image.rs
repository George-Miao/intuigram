//! Manual image-only Message probe for Intuigram's production Ghostty renderer.
//!
//! Run inside Ghostty:
//!
//! ```text
//! cargo run -p intuigram-tui --example ghostty_image
//! ```

use crossterm::event::{Event, KeyCode, KeyEventKind};
use intuigram_app::{
    App, ChatId, ChatKind, ChatView, DeliveryState, Focus, InlineImage, MediaCard, MediaKind,
    MediaPreviewView, MessageDetails, MessageDirection, MessageId, MessageView,
};
use intuigram_tui::TerminalUi;

const WIDTH: u16 = 96;
const HEIGHT: u16 = 48;

fn main() -> intuigram_tui::Result<()> {
    let mut view = App::new().view();
    view.chats.push(ChatView {
        id: ChatId(1),
        title: "Ghostty image probe".to_owned(),
        preview: String::new(),
        status: "Press q to exit".to_owned(),
        unread: 0,
        pinned: false,
        can_pin_messages: false,
        kind: ChatKind::Private,
        folders: vec![0],
    });
    view.active_chat = Some(0);
    view.focus = Focus::Composer;
    view.messages.push(MessageView {
        id: MessageId(1),
        sender: "Image probe".to_owned(),
        body: String::new(),
        timestamp: String::new(),
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
                remote_id: Some("probe".to_owned()),
            }),
            ..MessageDetails::default()
        },
    });
    view.media_previews.push(MediaPreviewView {
        chat: ChatId(1),
        message: MessageId(1),
        image: probe_image(),
    });

    let mut terminal = TerminalUi::enter()?;
    terminal.draw(&view)?;
    while !matches!(
        crossterm::event::read(),
        Ok(Event::Key(key)) if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q')
    ) {}
    Ok(())
}

fn probe_image() -> InlineImage {
    let mut rgba = Vec::with_capacity(usize::from(WIDTH) * usize::from(HEIGHT) * 4);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let color = match (x < WIDTH / 2, y < HEIGHT / 2) {
                (true, true) => [255, 70, 70, 255],
                (false, true) => [70, 220, 110, 255],
                (true, false) => [70, 130, 255, 255],
                (false, false) => [255, 220, 70, 255],
            };
            rgba.extend_from_slice(&color);
        }
    }
    InlineImage::from_rgba(WIDTH, HEIGHT, rgba)
        .expect("the probe dimensions exactly describe its RGBA pixels")
}
