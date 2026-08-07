use crossterm::event::{
    Event, KeyCode as CrosstermKey, KeyEvent, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags,
    MouseButton, MouseEvent, MouseEventKind,
};
use intuigram_app::{
    Action, ActivationTarget, ChatId, ChatKind, ChatLoadingState, ChatView, ComposerView,
    ConnectionState, DeliveryState, Focus, FolderView, MediaCard, MediaKind, MessageDetails,
    MessageDirection, MessageId, MessageView, PollOptionView, PollView, ReactionView, SearchView,
    TextEntity, TextEntityKind, View,
};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::{Color, Modifier};

use super::{
    EffectiveKeymap, Key, KeyChord, SemanticRole, UiEvent, ViewMode, chord_from_crossterm,
    qr_login_symbols, render, render_test_frame, render_with_mode, resolve_event,
    resolve_test_frame_event, terminal_keyboard_flags,
};

#[test]
fn qr_login_renderer_produces_a_compact_high_contrast_symbol() {
    let rendered = qr_login_symbols("tg://login?token=-_8").expect("login URI should fit a QR");
    let lines = rendered.dense.lines().collect::<Vec<_>>();

    assert!(lines.len() > 10);
    assert!(lines.len() < 30);
    assert!(lines.iter().any(|line| line.contains('█')));
    assert!(lines.iter().all(|line| line.chars().count() > 20));
}

#[test]
fn full_size_login_token_has_an_80_by_24_terminal_fallback() {
    let uri = format!("tg://login?token={}", "a".repeat(350));
    let rendered = qr_login_symbols(&uri).expect("login URI should fit a QR");
    let lines = rendered.compact.lines().collect::<Vec<_>>();

    assert!(lines.len() <= 20);
    assert!(
        lines
            .iter()
            .all(|line| line.chars().count() <= usize::from(80_u16))
    );
    assert!(
        lines
            .iter()
            .any(|line| line.chars().any(|ch| ch > '\u{2800}'))
    );
}

fn view(actions: Vec<Action>) -> View {
    View {
        connection: ConnectionState::Connected,
        account_name: "Test".to_owned(),
        notification_identity: "telegram:test".to_owned(),
        folders: Vec::new(),
        active_folder: 0,
        chats: Vec::new(),
        active_chat: None,
        messages: Vec::new(),
        chat_loading: ChatLoadingState::Idle,
        pinned_messages: Vec::new(),
        active_message: None,
        active_thread: None,
        transcript_anchor: None,
        unread_boundary: None,
        focus: Focus::Chats,
        composer: ComposerView::default(),
        search: None::<SearchView>,
        save_as: None,
        attachment_path: None,
        has_newer_messages: false,
        help_open: false,
        folder_picker: None,
        delete_confirmation: None,
        forward_picker: None,
        reaction_picker: None,
        poll_vote: None,
        link_confirmation: None,
        downloads: Vec::new(),
        media_previews: Vec::new(),
        media_preview_loads: Vec::new(),
        poll_composer: false,
        notice: None,
        animation_frame: 0,
        actions,
    }
}

#[test]
fn displayed_action_bar_and_help_bindings_are_the_bindings_input_resolves() {
    let current_view = view(vec![
        Action::Quit,
        Action::Search,
        Action::JumpLatest,
        Action::Help,
    ]);
    let keymap = EffectiveKeymap::defaults();

    for binding in keymap.help(&current_view) {
        assert_eq!(
            keymap.resolve(&current_view, binding.key),
            Some(binding.action)
        );
        assert!(!binding.key.label().is_empty());
    }
    assert_eq!(
        keymap.resolve(&current_view, KeyChord::control(Key::Char('f'))),
        Some(Action::Search)
    );
    assert_eq!(
        keymap.resolve(&current_view, KeyChord::control(Key::Char('c'))),
        Some(Action::Quit)
    );
    assert_eq!(
        keymap
            .action_bar(&current_view)
            .find(|binding| binding.action == Action::Quit)
            .map(|binding| binding.key),
        Some(KeyChord::control(Key::Char('c')))
    );
    assert_eq!(
        keymap.resolve(&current_view, KeyChord::shift(Key::Down)),
        Some(Action::JumpLatest)
    );
    assert_eq!(
        keymap.resolve(&current_view, KeyChord::plain(Key::End)),
        Some(Action::JumpLatest)
    );

    let composer = view(vec![
        Action::Send,
        Action::Newline,
        Action::Attach,
        Action::Paste,
    ]);
    assert_eq!(
        keymap.resolve(&composer, KeyChord::control(Key::Char('s'))),
        Some(Action::Send)
    );
    assert_eq!(
        keymap
            .action_bar(&composer)
            .find(|binding| binding.action == Action::Send)
            .map(|binding| binding.key),
        Some(KeyChord::plain(Key::Enter))
    );
    assert_eq!(
        keymap.resolve(&composer, KeyChord::shift(Key::Enter)),
        Some(Action::Newline)
    );
    assert_eq!(
        keymap.resolve(&composer, KeyChord::control(Key::Char('o'))),
        Some(Action::Attach)
    );
    assert_eq!(
        keymap
            .action_bar(&composer)
            .find(|binding| binding.action == Action::Attach)
            .map(|binding| (binding.key, binding.label)),
        Some((KeyChord::control(Key::Char('o')), "Attach"))
    );
    assert_eq!(
        keymap.resolve(&composer, KeyChord::control(Key::Char('v'))),
        Some(Action::Paste)
    );
    assert_eq!(
        keymap
            .action_bar(&composer)
            .find(|binding| binding.action == Action::Newline)
            .map(|binding| binding.key),
        Some(KeyChord::shift(Key::Enter))
    );
}

#[test]
fn link_and_download_keys_are_visible_only_when_contextually_available() {
    let current = view(vec![
        Action::OpenLink,
        Action::DownloadMedia,
        Action::SaveAs,
        Action::OpenDownload,
    ]);
    let keymap = EffectiveKeymap::defaults();

    assert_eq!(
        keymap.resolve(&current, KeyChord::control(Key::Char('l'))),
        Some(Action::OpenLink)
    );
    assert_eq!(
        keymap.resolve(&current, KeyChord::control(Key::Char('d'))),
        Some(Action::DownloadMedia)
    );
    assert_eq!(
        keymap.resolve(&current, KeyChord::alt(Key::Char('d'))),
        Some(Action::SaveAs)
    );
    assert_eq!(
        keymap.resolve(&current, KeyChord::control(Key::Char('o'))),
        Some(Action::OpenDownload)
    );
    assert_eq!(keymap.action_bar(&current).count(), 4);
}

#[test]
fn hierarchy_modifiers_resolve_only_in_their_effective_context() {
    let keymap = EffectiveKeymap::defaults();
    let chat_list = view(vec![
        Action::PreviousFolder,
        Action::NextFolder,
        Action::ManageFolders,
        Action::Open,
    ]);
    assert_eq!(
        keymap.resolve(&chat_list, KeyChord::plain(Key::Left)),
        Some(Action::PreviousFolder)
    );
    assert_eq!(
        keymap.resolve(&chat_list, KeyChord::plain(Key::Right)),
        Some(Action::NextFolder)
    );
    assert_eq!(
        keymap.resolve(&chat_list, KeyChord::alt(Key::Char('f'))),
        Some(Action::ManageFolders)
    );

    let picker = view(vec![Action::ToggleFolderMembership, Action::Cancel]);
    assert_eq!(
        keymap.resolve(&picker, KeyChord::plain(Key::Enter)),
        Some(Action::ToggleFolderMembership)
    );

    let mut composer = view(vec![Action::TargetPreviousMessage, Action::Cancel]);
    composer.focus = Focus::Composer;
    assert_eq!(keymap.resolve(&composer, KeyChord::plain(Key::Up)), None);
    let mut empty_editor = view(vec![Action::EditPrevious]);
    empty_editor.focus = Focus::Composer;
    assert_eq!(
        keymap.resolve(&empty_editor, KeyChord::plain(Key::Up)),
        Some(Action::EditPrevious)
    );
    assert_eq!(
        keymap.resolve(&composer, KeyChord::alt(Key::Up)),
        Some(Action::TargetPreviousMessage)
    );
    composer.composer.text = "draft".to_owned();
    assert_eq!(
        keymap.resolve(&composer, KeyChord::alt(Key::Up)),
        Some(Action::TargetPreviousMessage)
    );
    assert_eq!(keymap.resolve(&composer, KeyChord::plain(Key::Up)), None);
    assert_eq!(keymap.resolve(&composer, KeyChord::alt(Key::Left)), None);
    assert_eq!(keymap.resolve(&composer, KeyChord::alt(Key::Right)), None);
    assert!(chord_from_crossterm(CrosstermKey::Tab, KeyModifiers::NONE).is_none());
}

#[test]
fn terminal_events_resolve_against_the_current_view() {
    let current_view = view(vec![Action::Quit, Action::Search]);
    let keymap = EffectiveKeymap::defaults();

    assert_eq!(
        resolve_event(
            &keymap,
            &current_view,
            Event::Key(KeyEvent::new_with_kind(
                CrosstermKey::Char('c'),
                KeyModifiers::CONTROL,
                KeyEventKind::Press,
            )),
        ),
        Some(UiEvent::Intent(intuigram_app::Intent::Action(Action::Quit)))
    );
    assert_eq!(
        resolve_event(
            &keymap,
            &current_view,
            Event::Key(KeyEvent::new_with_kind(
                CrosstermKey::Char('f'),
                KeyModifiers::CONTROL,
                KeyEventKind::Press,
            )),
        ),
        Some(UiEvent::Intent(intuigram_app::Intent::Action(
            Action::Search
        )))
    );
    assert_eq!(
        resolve_event(&keymap, &current_view, Event::Paste("hello".to_owned())),
        Some(UiEvent::Intent(intuigram_app::Intent::Insert(
            "hello".to_owned()
        )))
    );
    assert_eq!(
        resolve_event(&keymap, &current_view, Event::Resize(100, 30)),
        Some(UiEvent::Redraw)
    );

    let mut composer = view(vec![Action::Send, Action::Newline]);
    composer.focus = Focus::Composer;
    assert_eq!(
        resolve_event(
            &keymap,
            &composer,
            Event::Key(KeyEvent::new_with_kind(
                CrosstermKey::Enter,
                KeyModifiers::NONE,
                KeyEventKind::Press,
            )),
        ),
        Some(UiEvent::Intent(intuigram_app::Intent::Action(Action::Send)))
    );
    assert_eq!(
        resolve_event(
            &keymap,
            &composer,
            Event::Key(KeyEvent::new_with_kind(
                CrosstermKey::Enter,
                KeyModifiers::SHIFT,
                KeyEventKind::Press,
            )),
        ),
        Some(UiEvent::Intent(intuigram_app::Intent::Action(
            Action::Newline
        )))
    );
}

#[test]
fn primary_clicks_resolve_from_the_matching_rendered_semantics() {
    let mut current = view(Vec::new());
    current.folders.push(FolderView {
        id: 3,
        title: "Work".to_owned(),
        unread: 0,
    });
    current.chats.push(ChatView {
        id: ChatId(7),
        title: "Ada".to_owned(),
        preview: "hello".to_owned(),
        status: "online".to_owned(),
        unread: 0,
        pinned: false,
        can_pin_messages: true,
        kind: ChatKind::Private,
        folders: vec![3],
    });
    current.active_chat = Some(0);
    current.messages.push(MessageView {
        id: MessageId(11),
        sender: "Ada".to_owned(),
        body: "hello".to_owned(),
        timestamp: "12:00".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: None,
        details: MessageDetails::default(),
    });
    let frame = render_test_frame(&current, 120, 40);
    let targets = [
        (SemanticRole::Folder, ActivationTarget::Folder(3)),
        (SemanticRole::Chat, ActivationTarget::Chat(ChatId(7))),
        (
            SemanticRole::Message,
            ActivationTarget::Message(MessageId(11)),
        ),
        (SemanticRole::Composer, ActivationTarget::Composer),
    ];

    for (role, target) in targets {
        let node = frame
            .semantics
            .iter()
            .find(|node| node.role == role)
            .expect("rendered target should have semantic bounds");
        let event = Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: node.bounds.x,
            row: node.bounds.y,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(
            resolve_test_frame_event(&current, &frame, event),
            Some(UiEvent::Intent(intuigram_app::Intent::Activate(target)))
        );
    }
}

#[test]
fn modified_clicks_remain_available_to_the_terminal() {
    let current = view(Vec::new());
    let frame = render_test_frame(&current, 120, 40);

    assert_eq!(
        resolve_test_frame_event(
            &current,
            &frame,
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 0,
                row: 0,
                modifiers: KeyModifiers::SHIFT,
            }),
        ),
        None
    );
}
mod avatars;
mod density;
mod effort;
mod forwarded;
mod image_loading;
mod loading;
mod metadata;
mod multiline;
mod rendering;
mod semantics;
mod transcript_refinement;
mod unread;
