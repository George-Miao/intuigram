use std::fmt::Write as _;
use std::io::{self, Stdout};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use compio_term::EventStream;
use crossterm::event::{
    self, Event, KeyCode as CrosstermKey, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags,
    PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use futures_util::{Stream, StreamExt};
use intuigram_app::{
    Action, ConnectionState, DeliveryState, Focus, Intent, MessageDirection, MessageView,
    TextEntityKind, View,
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

mod events;
mod key_chord;
pub(crate) mod qr_render;
mod qr_session;
mod render_chrome;
pub(crate) mod render_layout;
mod render_overlays;
mod render_transcript;
pub(crate) mod terminal;
mod view_mode;

pub use events::*;
use qr_render::{chord_from_crossterm, qr_login_symbols, render_qr_login};
pub use qr_session::*;
use render_chrome::{
    anchored_window, centered_rect, render_actions, render_composer, render_folder_picker,
    render_folders, render_help, render_status, selection_rule, surface_style,
};
use render_layout::{render_with_mode, render_with_semantics};
use render_overlays::{
    render_delete_confirmation, render_forward_picker, render_link_confirmation, render_poll_vote,
    render_reaction_picker,
};
use render_transcript::render_transcript;
pub use terminal::*;
use terminal::{enter_terminal, restore_terminal};
pub use view_mode::ViewMode;

// OpenCode's Everforest light palette, kept local until themes become
// configurable.
pub(crate) const BACKGROUND: Color = Color::Rgb(253, 246, 227);
pub(crate) const SURFACE_BACKGROUND: Color = Color::Rgb(244, 240, 217);
const FOCUSED_SURFACE_BACKGROUND: Color = Color::Rgb(230, 226, 204);
pub(crate) const CHROME_BACKGROUND: Color = Color::Rgb(239, 235, 212);
pub(crate) const TEXT: Color = Color::Rgb(92, 106, 114);
pub(crate) const MUTED_TEXT: Color = Color::Rgb(130, 145, 129);
pub(crate) const PRIMARY: Color = Color::Rgb(141, 161, 1);
const SECONDARY: Color = Color::Rgb(58, 148, 197);

/// A terminal key independent of a concrete terminal backend.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Key {
    /// Printable character.
    Char(char),
    /// Up arrow.
    Up,

    /// Down arrow.
    Down,

    /// Left arrow.
    Left,

    /// Right arrow.
    Right,

    /// Home key.
    Home,

    /// End key.
    End,

    /// Enter key.
    Enter,

    /// Escape key.
    Escape,
}

/// A terminal key with modifier state.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct KeyChord {
    key: Key,
    control: bool,
    shift: bool,
    alt: bool,
}

/// One context-sensitive shortcut shown in the Action Bar and Help.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Binding {
    /// Shortcut accepted by input handling.
    pub key: KeyChord,
    /// User-facing action label.
    pub label: &'static str,
    /// Application action produced by the shortcut.
    pub action: Action,
    /// Whether this is the compact Action Bar binding for its action.
    pub primary: bool,
}

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
        KeyChord::alt(Key::Left),
        "Previous Folder",
        Action::PreviousFolder,
        true,
    ),
    binding(
        KeyChord::alt(Key::Right),
        "Next Folder",
        Action::NextFolder,
        true,
    ),
    binding(
        KeyChord::alt(Key::Char('f')),
        "Manage Folders",
        Action::ManageFolders,
        true,
    ),
    binding(KeyChord::alt(Key::Char('r')), "React", Action::React, true),
    binding(
        KeyChord::plain(Key::Char('v')),
        "Vote",
        Action::VotePoll,
        true,
    ),
    binding(
        KeyChord::plain(Key::Char(' ')),
        "Toggle Choice",
        Action::TogglePollChoice,
        true,
    ),
    binding(
        KeyChord::plain(Key::Enter),
        "Submit Vote",
        Action::ConfirmPollVote,
        true,
    ),
    binding(
        KeyChord::control(Key::Char('l')),
        "Open Link",
        Action::OpenLink,
        true,
    ),
    binding(
        KeyChord::plain(Key::Enter),
        "Open Destination",
        Action::ConfirmOpenLink,
        true,
    ),
    binding(
        KeyChord::control(Key::Char('d')),
        "Download",
        Action::DownloadMedia,
        true,
    ),
    binding(
        KeyChord::control(Key::Char('o')),
        "Open Download",
        Action::OpenDownload,
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
        KeyChord::control(Key::Char('v')),
        "Paste",
        Action::Paste,
        true,
    ),
    binding(
        KeyChord::control(Key::Char('p')),
        "Poll",
        Action::CreatePoll,
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
        KeyChord::control(Key::Char('r')),
        "Reply",
        Action::Reply,
        true,
    ),
    binding(KeyChord::alt(Key::Char('e')), "Edit", Action::Edit, true),
    binding(
        KeyChord::alt(Key::Char('d')),
        "Delete",
        Action::Delete,
        true,
    ),
    binding(
        KeyChord::alt(Key::Char('f')),
        "Forward",
        Action::Forward,
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
        KeyChord::control(Key::Char('t')),
        "Thread",
        Action::OpenThread,
        true,
    ),
    binding(
        KeyChord::alt(Key::Up),
        "Previous Message",
        Action::TargetPreviousMessage,
        true,
    ),
    binding(
        KeyChord::alt(Key::Down),
        "Next Message",
        Action::TargetNextMessage,
        true,
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

impl EffectiveKeymap {
    /// Creates the built-in keymap.
    #[must_use]
    pub const fn defaults() -> Self {
        Self
    }

    /// Resolves a key only when its action is valid in the current view.
    #[must_use]
    pub fn resolve(&self, view: &View, key: KeyChord) -> Option<Action> {
        self.help(view)
            .find(|binding| binding.key == key)
            .map(|binding| binding.action)
    }

    /// Produces compact Action Bar bindings.
    pub fn action_bar<'a>(&'a self, view: &'a View) -> impl Iterator<Item = &'static Binding> + 'a {
        self.help(view).filter(|binding| binding.primary)
    }

    /// Produces exhaustive context Help from the same bindings used for input.
    pub fn help<'a>(&'a self, view: &'a View) -> impl Iterator<Item = &'static Binding> + 'a {
        BINDINGS
            .iter()
            .filter(|binding| view.actions.contains(&binding.action))
    }
}

impl Default for EffectiveKeymap {
    fn default() -> Self {
        Self::defaults()
    }
}
