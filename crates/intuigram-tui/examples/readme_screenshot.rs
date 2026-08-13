//! Reproducible README screenshot rendered through Intuigram's production TUI.

use crossterm::event::{Event, KeyCode, KeyEventKind};
use intuigram_lib::{
    Action, App, AvatarId, AvatarRef, AvatarView, ChatId, ChatKind, ChatView, ComposerView,
    ConnectionState, DeliveryState, Focus, FolderView, InlineImage, MediaCard, MediaKind,
    MediaPreviewView, MessageDetails, MessageDirection, MessageId, MessageView, ReactionView,
};
use intuigram_tui::TerminalUi;

const STUDIO: ChatId = ChatId(100);
const NORA: ChatId = ChatId(201);
const KAI: ChatId = ChatId(202);
const MIRA: ChatId = ChatId(203);

fn main() -> intuigram_tui::Result<()> {
    let mut view = App::new().view();
    view.connection = ConnectionState::Connected;
    view.account_name = "Maya Chen".to_owned();
    view.notification_identity = "telegram:demo".to_owned();
    view.folders = vec![
        folder(0, "All", 7),
        folder(1, "Work", 3),
        folder(2, "Friends", 4),
        folder(-1, "Archive", 0),
    ];
    view.active_folder = 1;
    view.chats = demo_chats();
    view.active_chat = Some(0);
    view.focus = Focus::Composer;
    view.composer = ComposerView::default();
    view.messages = demo_messages();
    view.avatars = demo_avatars();
    view.media_previews = vec![MediaPreviewView {
        chat: STUDIO,
        message: MessageId(3),
        image: sticker(),
    }];
    view.actions = vec![
        Action::Send,
        Action::Help,
        Action::Quit,
        Action::OpenActions,
        Action::Newline,
        Action::Search,
        Action::Cancel,
    ];

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

fn folder(id: i32, title: &str, unread: u32) -> FolderView {
    FolderView {
        id,
        title: title.to_owned(),
        unread,
    }
}

fn demo_chats() -> Vec<ChatView> {
    vec![
        chat(
            STUDIO,
            "Intuigram Studio",
            "Sticker pass is ready ✨",
            Some(("Kai Morgan", KAI)),
            "10:42",
            "8 members · 3 online",
            0,
            true,
            ChatKind::Supergroup,
        ),
        chat(
            ChatId(110),
            "Rust Async",
            "The backpressure notes look solid.",
            Some(("Nora Park", NORA)),
            "10:18",
            "24 members",
            3,
            true,
            ChatKind::Supergroup,
        ),
        chat(
            ChatId(120),
            "Product Garden",
            "Mira: shipped the new build",
            None,
            "09:55",
            "5 members",
            0,
            false,
            ChatKind::Supergroup,
        ),
        chat(
            ChatId(130),
            "Release Notes",
            "Intuigram 0.4.0 candidate",
            None,
            "09:31",
            "1.2K subscribers",
            4,
            false,
            ChatKind::Channel,
        ),
        chat(
            ChatId(140),
            "Weekend plans",
            "Coffee before the train?",
            None,
            "Yesterday",
            "online",
            0,
            false,
            ChatKind::Private,
        ),
        chat(
            ChatId(150),
            "Saved Messages",
            "release checklist and links",
            None,
            "Mon",
            "",
            0,
            false,
            ChatKind::SavedMessages,
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn chat(
    id: ChatId,
    title: &str,
    preview: &str,
    sender: Option<(&str, ChatId)>,
    timestamp: &str,
    status: &str,
    unread: u32,
    pinned: bool,
    kind: ChatKind,
) -> ChatView {
    ChatView {
        id,
        title: title.to_owned(),
        preview: preview.to_owned(),
        preview_sender: sender.map(|(name, _)| name.to_owned()),
        preview_sender_peer: sender.map(|(_, peer)| peer),
        preview_timestamp: timestamp.to_owned(),
        status: status.to_owned(),
        unread,
        pinned,
        can_pin_messages: true,
        has_topics: false,
        has_direct_messages: false,
        kind,
        folders: vec![0, 1],
    }
}

fn demo_messages() -> Vec<MessageView> {
    vec![
        message(
            1,
            "Nora Park",
            NORA,
            "The new terminal image path is in. Avatars stay crisp without giving up the dense \
             transcript.",
            "10:06",
            MessageDirection::Incoming,
        )
        .with_date("Today"),
        message(
            2,
            "You",
            ChatId(999),
            "Nice. This finally feels like a Telegram client built for the terminal.",
            "10:11",
            MessageDirection::Outgoing,
        )
        .with_reply(MessageId(1)),
        message(
            3,
            "Kai Morgan",
            KAI,
            "[sticker.webp]",
            "10:14",
            MessageDirection::Incoming,
        )
        .with_sticker(),
        message(
            4,
            "Mira Shah",
            MIRA,
            "I tightened the spacing and kept the keyboard map visible. Ready for the README.",
            "10:17",
            MessageDirection::Incoming,
        )
        .with_reaction("🔥", 3),
    ]
}

struct MessageBuilder(MessageView);

impl MessageBuilder {
    fn with_date(mut self, date: &str) -> MessageView {
        self.0.details.date_label = date.to_owned();
        self.0
    }

    fn with_reply(mut self, reply: MessageId) -> MessageView {
        self.0.reply_to = Some(reply);
        self.0
    }

    fn with_sticker(mut self) -> MessageView {
        self.0.details.media = Some(MediaCard {
            kind: MediaKind::Sticker,
            title: "sticker.webp".to_owned(),
            description: String::new(),
            details: Vec::new(),
            poll: None,
            specialized: None,
            remote_id: Some("demo-sticker".to_owned()),
        });
        self.0
    }

    fn with_reaction(mut self, label: &str, count: u32) -> MessageView {
        self.0.details.reactions.push(ReactionView {
            label: label.to_owned(),
            count,
            chosen: true,
        });
        self.0
    }
}

fn message(
    id: i64,
    sender: &str,
    peer: ChatId,
    body: &str,
    timestamp: &str,
    direction: MessageDirection,
) -> MessageBuilder {
    MessageBuilder(MessageView {
        id: MessageId(id),
        sender: sender.to_owned(),
        body: body.to_owned(),
        timestamp: timestamp.to_owned(),
        direction,
        delivery: DeliveryState::Read,
        reply_to: None,
        details: MessageDetails {
            sender_peer: Some(peer),
            ..MessageDetails::default()
        },
    })
}

fn demo_avatars() -> Vec<AvatarView> {
    [
        (
            STUDIO,
            image(include_bytes!("assets/avatar-laundromat.jpg")),
        ),
        (NORA, image(include_bytes!("assets/avatar-white.jpg"))),
        (KAI, image(include_bytes!("assets/avatar-blonde.jpg"))),
        (MIRA, image(include_bytes!("assets/avatar-sunset.jpg"))),
        (
            ChatId(110),
            image(include_bytes!("assets/avatar-kimono.jpg")),
        ),
        (
            ChatId(120),
            image(include_bytes!("assets/avatar-guitar.jpg")),
        ),
        (ChatId(130), image(include_bytes!("assets/avatar-rose.jpg"))),
        (
            ChatId(140),
            image(include_bytes!("assets/avatar-sunset.jpg")),
        ),
        (ChatId(150), image(include_bytes!("assets/avatar-rose.jpg"))),
        (ChatId(999), image(include_bytes!("assets/avatar-rose.jpg"))),
    ]
    .into_iter()
    .map(|(peer, image)| AvatarView {
        avatar: AvatarRef {
            peer,
            id: AvatarId(peer.0),
        },
        image,
    })
    .collect()
}

fn sticker() -> InlineImage {
    image(include_bytes!("assets/llm-moe.png"))
}

fn image(bytes: &[u8]) -> InlineImage {
    let decoded = image::load_from_memory(bytes)
        .expect("embedded screenshot artwork should decode")
        .into_rgba8();
    let width = u16::try_from(decoded.width()).expect("embedded artwork width should fit u16");
    let height = u16::try_from(decoded.height()).expect("embedded artwork height should fit u16");
    InlineImage::from_rgba(width, height, decoded.into_raw())
        .expect("embedded artwork dimensions should match its RGBA pixels")
}

fn wait_for_graphics(terminal: &mut TerminalUi) -> intuigram_tui::Result<()> {
    loop {
        let waker = futures_util::task::noop_waker();
        let mut context = std::task::Context::from_waker(&waker);
        match terminal.poll_redraw(&mut context) {
            std::task::Poll::Ready(result) => return result,
            std::task::Poll::Pending => std::thread::sleep(std::time::Duration::from_millis(5)),
        }
    }
}
