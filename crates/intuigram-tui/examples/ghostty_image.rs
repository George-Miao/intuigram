//! Manual image-only Message probe for Intuigram's production Ghostty renderer.
//!
//! Run inside Ghostty:
//!
//! ```text
//! cargo run -p intuigram-tui --example ghostty_image -- /path/to/detailed.png
//! ```

use crossterm::event::{Event, KeyCode, KeyEventKind};
use intuigram_lib::{
    App, ChatId, ChatKind, ChatView, DeliveryState, Focus, InlineImage, MediaCard, MediaKind,
    MediaPreviewView, MessageDetails, MessageDirection, MessageId, MessageView,
};
use intuigram_tui::TerminalUi;

fn main() -> intuigram_tui::Result<()> {
    let image_path = std::env::args()
        .nth(1)
        .expect("the Ghostty image probe requires an image path");
    let mut view = App::new().view();
    view.chats.push(ChatView {
        id: ChatId(1),
        title: "Ghostty image probe".to_owned(),
        preview: String::new(),
        preview_sender: None,
        preview_sender_peer: None,
        preview_timestamp: String::new(),
        status: "Press q to exit".to_owned(),
        unread: 0,
        pinned: false,
        can_pin_messages: false,
        has_topics: false,
        has_direct_messages: false,
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
            sender_peer: None,
            media: Some(MediaCard {
                kind: MediaKind::Photo,
                title: "Photo".to_owned(),
                description: "image".to_owned(),
                details: Vec::new(),
                poll: None,
                specialized: None,
                remote_id: Some("probe".to_owned()),
            }),
            ..MessageDetails::default()
        },
    });
    view.media_previews.push(MediaPreviewView {
        chat: ChatId(1),
        message: MessageId(1),
        image: probe_image(&image_path),
    });

    let mut terminal = TerminalUi::enter()?;
    terminal.draw(&view)?;
    wait_for_graphics(&mut terminal)?;
    terminal.draw(&view)?;
    while !matches!(
        crossterm::event::read(),
        Ok(Event::Key(key)) if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q')
    ) {}
    Ok(())
}

fn wait_for_graphics(terminal: &mut TerminalUi) -> intuigram_tui::Result<()> {
    loop {
        let waker = futures_util::task::noop_waker();
        let mut context = std::task::Context::from_waker(&waker);
        match terminal.poll_redraw(&mut context) {
            std::task::Poll::Ready(result) => return result,
            std::task::Poll::Pending => {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
    }
}

fn probe_image(path: &str) -> InlineImage {
    let decoded = image::open(path)
        .expect("the Ghostty probe image should decode")
        .resize(640, 640, image::imageops::FilterType::Lanczos3)
        .into_rgba8();
    let width = u16::try_from(decoded.width()).expect("the resized probe width fits in u16");
    let height = u16::try_from(decoded.height()).expect("the resized probe height fits in u16");
    InlineImage::from_rgba(width, height, decoded.into_raw())
        .expect("the decoded probe dimensions exactly describe its RGBA pixels")
}
