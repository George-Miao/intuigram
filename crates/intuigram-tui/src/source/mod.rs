use std::io::{self, Stdout};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use compio_term::EventStream;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode as CrosstermKey, KeyEventKind,
    KeyModifiers, KeyboardEnhancementFlags, MouseButton, MouseEventKind,
    PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode, window_size,
};
use futures_util::{Stream, StreamExt};
use intuigram_app::{
    Action, ActivationTarget, ChatId, ChatKind, ChatLoadingState, ChatView, ComposerMovement,
    ConnectionState, DeliveryState, Focus, Intent, MessageDirection, MessageId, MessageView,
    ScrollDirection, ScrollTarget, SearchScope, TextEntityKind, TopicId, View,
};
use qrcode::render::unicode::Dense1x2;
use qrcode::types::Color as QrColor;
use qrcode::{EcLevel, QrCode};
use ratatui::backend::{CrosstermBackend, TestBackend};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, List, ListItem, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use snafu::{OptionExt, ResultExt, Snafu};

use crate::recovery::{self, RecoveryView};

mod avatar;
mod composer_wrap;
mod effort;
mod events;
pub(crate) mod graphics;
mod key_chord;
mod login;
mod palette;
mod pointer;
pub(crate) mod qr_render;
mod qr_session;
mod render_accounts;
mod render_chrome;
mod render_composer;
mod render_details;
mod render_folder_manager;
mod render_headers;
pub(crate) mod render_layout;
mod render_outbox;
mod render_overlays;
mod render_rich_media;
mod render_saved_dialogs;
mod render_scheduled;
pub(crate) mod render_text;
mod render_topics;
mod render_transcript;
pub(crate) mod terminal;
mod test_renderer;
mod view_mode;

use avatar::{avatar_block, avatar_spans, avatar_width};
use effort::effort_spans;
pub use events::*;
pub use graphics::Error as GraphicsError;
use graphics::{GraphicsFrame, GraphicsProtocol, GraphicsWorker, avatar_image_id, image_id};
pub use key_chord::{Binding, Key, KeyChord};
pub use login::{LoginField, LoginInput, LoginPrompt, LoginUi};
pub(crate) use palette::*;
use pointer::resolve_pointer;
use qr_render::{chord_from_crossterm, qr_login_symbols, render_qr_login};
pub use qr_session::*;
use render_accounts::{render_account_confirmation, render_account_picker};
pub(crate) use render_chrome::ChatViewport;
use render_chrome::{
    centered_rect, interaction_rule, render_bottom_chrome, render_folder_picker, render_folders,
    render_help, selection_rule, surface_style,
};
use render_composer::{composer_cursor_at, composer_height, render_composer};
use render_details::render_thread_details;
use render_folder_manager::render_folder_manager;
use render_headers::{render_active_chat_header, render_chat_list_header};
use render_layout::render_with_graphics;
use render_overlays::{
    render_action_menu, render_attachment_path, render_delete_confirmation, render_forward_picker,
    render_link_confirmation, render_poll_vote, render_reaction_picker, render_save_as,
    render_todo_editor,
};
use render_rich_media::render_rich_media;
use render_saved_dialogs::render_saved_dialogs;
use render_scheduled::render_scheduled;
use render_text::capped_text;
use render_topics::render_topics;
use render_transcript::render_transcript;
pub use terminal::*;
use terminal::{enter_terminal, restore_terminal};
pub use test_renderer::*;
pub use view_mode::{ViewMode, ViewOptions};

const BINDINGS: &[Binding] = &[
    binding(
        KeyChord::control(Key::Char('c')),
        "Quit",
        Action::Quit,
        true,
    ),
    binding(
        KeyChord::control(Key::Char('q')),
        "Quit",
        Action::Quit,
        false,
    ),
    binding(KeyChord::plain(Key::Char('?')), "Help", Action::Help, true),
    binding(KeyChord::plain(Key::Up), "Up", Action::MoveUp, true),
    binding(KeyChord::plain(Key::Down), "Down", Action::MoveDown, true),
    binding(
        KeyChord::plain(Key::Left),
        "Previous Folder",
        Action::PreviousFolder,
        true,
    ),
    binding(
        KeyChord::plain(Key::Right),
        "Next Folder",
        Action::NextFolder,
        true,
    ),
    binding(
        KeyChord::alt(Key::Left),
        "Previous Folder",
        Action::PreviousFolder,
        false,
    ),
    binding(
        KeyChord::alt(Key::Right),
        "Next Folder",
        Action::NextFolder,
        false,
    ),
    binding(
        KeyChord::alt(Key::Char('f')),
        "Manage Folders",
        Action::ManageFolders,
        true,
    ),
    binding(
        KeyChord::plain(Key::Char(' ')),
        "Toggle Choice",
        Action::TogglePollChoice,
        true,
    ),
    binding(
        KeyChord::plain(Key::Char(' ')),
        "Toggle TODO",
        Action::ToggleTodoItem,
        true,
    ),
    binding(
        KeyChord::plain(Key::Char('a')),
        "Append TODO",
        Action::AppendTodoItem,
        true,
    ),
    binding(
        KeyChord::plain(Key::Enter),
        "Submit Vote",
        Action::ConfirmPollVote,
        true,
    ),
    binding(
        KeyChord::plain(Key::Enter),
        "Append Item",
        Action::ConfirmTodoAppend,
        true,
    ),
    binding(
        KeyChord::plain(Key::Enter),
        "Open Destination",
        Action::ConfirmOpenLink,
        true,
    ),
    binding(
        KeyChord::plain(Key::Enter),
        "Save Here",
        Action::ConfirmSaveAs,
        true,
    ),
    binding(
        KeyChord::plain(Key::Enter),
        "Apply Reaction",
        Action::ConfirmReaction,
        true,
    ),
    binding(
        KeyChord::plain(Key::Enter),
        "Toggle Folder",
        Action::ToggleFolderMembership,
        true,
    ),
    binding(KeyChord::plain(Key::Enter), "Open", Action::Open, true),
    binding(
        KeyChord::plain(Key::Char('a')),
        "Actions",
        Action::OpenActions,
        true,
    ),
    binding(
        KeyChord::alt(Key::Char('a')),
        "Actions",
        Action::OpenActions,
        true,
    ),
    binding(
        KeyChord::plain(Key::Enter),
        "Choose Action",
        Action::ChooseAction,
        true,
    ),
    binding(
        KeyChord::control(Key::Char('n')),
        "Draft",
        Action::Compose,
        true,
    ),
    binding(KeyChord::plain(Key::Enter), "Send", Action::Send, true),
    binding(
        KeyChord::shift(Key::Enter),
        "New Line",
        Action::Newline,
        true,
    ),
    binding(
        KeyChord::plain(Key::Enter),
        "Add Attachment",
        Action::ConfirmAttachment,
        true,
    ),
    binding(
        KeyChord::plain(Key::Enter),
        "Send Poll",
        Action::SendPoll,
        true,
    ),
    binding(
        KeyChord::control(Key::Enter),
        "Send (enhanced terminal)",
        Action::Send,
        false,
    ),
    binding(
        KeyChord::control(Key::Char('s')),
        "Send",
        Action::Send,
        false,
    ),
    binding(
        KeyChord::plain(Key::Up),
        "Edit Previous",
        Action::EditPrevious,
        true,
    ),
    binding(
        KeyChord::plain(Key::Enter),
        "Forward Here",
        Action::ConfirmForward,
        true,
    ),
    binding(
        KeyChord::plain(Key::Enter),
        "Confirm Delete",
        Action::ConfirmDelete,
        true,
    ),
    binding(
        KeyChord::plain(Key::Enter),
        "Save Edit",
        Action::SaveEdit,
        true,
    ),
    binding(
        KeyChord::plain(Key::Char('p')),
        "Pinned",
        Action::NavigatePinned,
        true,
    ),
    binding(
        KeyChord::plain(Key::Up),
        "Previous Message",
        Action::TargetPreviousMessage,
        true,
    ),
    binding(
        KeyChord::plain(Key::Down),
        "Next Message",
        Action::TargetNextMessage,
        true,
    ),
    binding(
        KeyChord::alt(Key::Up),
        "Previous Message",
        Action::TargetPreviousMessage,
        false,
    ),
    binding(
        KeyChord::alt(Key::Down),
        "Next Message",
        Action::TargetNextMessage,
        false,
    ),
    binding(
        KeyChord::control(Key::Char('f')),
        "Search",
        Action::Search,
        true,
    ),
    binding(KeyChord::plain(Key::Escape), "Back", Action::Cancel, true),
    binding(
        KeyChord::shift(Key::Up),
        "Earliest",
        Action::JumpEarliest,
        true,
    ),
    binding(
        KeyChord::plain(Key::Home),
        "Earliest",
        Action::JumpEarliest,
        false,
    ),
    binding(
        KeyChord::shift(Key::Down),
        "Latest",
        Action::JumpLatest,
        true,
    ),
    binding(
        KeyChord::plain(Key::End),
        "Latest",
        Action::JumpLatest,
        false,
    ),
    binding(
        KeyChord::alt(Key::Char('r')),
        "Reconnect",
        Action::Reconnect,
        true,
    ),
];

const fn binding(key: KeyChord, label: &'static str, action: Action, primary: bool) -> Binding {
    Binding {
        key,
        label,
        action,
        primary,
    }
}

/// Effective bindings for the active configuration.
pub struct EffectiveKeymap;
