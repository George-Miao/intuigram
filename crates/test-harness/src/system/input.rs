//! Named terminal inputs used by behavior scenarios.

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use intuigram_lib::Focus;

use super::TestSystem;
use crate::error::{Error, Result};

pub mod key {
    use super::TestKey;

    pub const ENTER: TestKey = TestKey::Enter;
    pub const ESCAPE: TestKey = TestKey::Escape;
    pub const UP: TestKey = TestKey::Up;
    pub const DOWN: TestKey = TestKey::Down;
    pub const LEFT: TestKey = TestKey::Left;
    pub const RIGHT: TestKey = TestKey::Right;
    pub const ALT_UP: TestKey = TestKey::AltUp;
    pub const ALT_DOWN: TestKey = TestKey::AltDown;
    pub const ALT_EDIT: TestKey = TestKey::AltEdit;
    pub const ALT_LEFT: TestKey = TestKey::AltLeft;
    pub const ALT_RIGHT: TestKey = TestKey::AltRight;
    pub const ALT_DELETE: TestKey = TestKey::AltDelete;
    pub const ALT_FORWARD: TestKey = TestKey::AltForward;
    pub const ALT_REACT: TestKey = TestKey::AltReact;
    pub const ALT_ACTIONS: TestKey = TestKey::AltActions;
    pub const SUPER_PASTE: TestKey = TestKey::SuperPaste;
    pub const CTRL_PASTE: TestKey = TestKey::ControlPaste;
    pub const ALT_PIN: TestKey = TestKey::AltPin;
    pub const PINNED: TestKey = TestKey::Pinned;
    pub const CTRL_POLL: TestKey = TestKey::ControlPoll;
    pub const CTRL_MEDIA: TestKey = TestKey::ControlMedia;
    pub const CTRL_SCHEDULED: TestKey = TestKey::ControlScheduled;
    pub const RESCHEDULE: TestKey = TestKey::Reschedule;
    pub const NEW_SCHEDULED: TestKey = TestKey::NewFolder;
    pub const EDIT_SCHEDULED: TestKey = TestKey::EditFolder;
    pub const DELETE_SCHEDULED: TestKey = TestKey::DeleteFolder;
    pub const SEND_NOW: TestKey = TestKey::ShareFolder;
    pub const VOTE: TestKey = TestKey::Vote;
    pub const SPACE: TestKey = TestKey::Space;
    pub const SHIFT_ENTER: TestKey = TestKey::ShiftEnter;
    pub const BACKSPACE: TestKey = TestKey::Backspace;
    pub const ALT_RECONNECT: TestKey = TestKey::AltReconnect;
    pub const CTRL_REPLY: TestKey = TestKey::ControlReply;
    pub const CTRL_THREAD: TestKey = TestKey::ControlThread;
    pub const CTRL_LINK: TestKey = TestKey::ControlLink;
    pub const CTRL_DOWNLOAD: TestKey = TestKey::ControlDownload;
    pub const CTRL_OPEN: TestKey = TestKey::ControlOpen;
    pub const REMOVE_ATTACHMENT: TestKey = TestKey::ControlDownload;
    pub const ACCOUNTS: TestKey = TestKey::Accounts;
    pub const ALT_LOGOUT: TestKey = TestKey::AltLogout;
    pub const ALT_REMOVE_LOCAL: TestKey = TestKey::AltRemoveLocal;
    pub const FOLDER_SETTINGS: TestKey = TestKey::FolderSettings;
    pub const NEW_FOLDER: TestKey = TestKey::NewFolder;
    pub const EDIT_FOLDER: TestKey = TestKey::EditFolder;
    pub const SHARE_FOLDER: TestKey = TestKey::ShareFolder;
    pub const DELETE_FOLDER: TestKey = TestKey::DeleteFolder;
    pub const SHIFT_UP: TestKey = TestKey::ShiftUp;
    pub const SHIFT_DOWN: TestKey = TestKey::ShiftDown;
}

#[derive(Clone, Copy, Debug)]
pub enum TestKey {
    Enter,
    Escape,
    Up,
    Down,
    Left,
    Right,
    AltUp,
    AltDown,
    AltLeft,
    AltRight,
    AltEdit,
    AltDelete,
    AltForward,
    AltReact,
    AltActions,
    SuperPaste,
    ControlPaste,
    AltPin,
    Pinned,
    ControlPoll,
    ControlMedia,
    ControlScheduled,
    Reschedule,
    Vote,
    Space,
    ShiftEnter,
    Backspace,
    AltReconnect,
    ControlReply,
    ControlThread,
    ControlLink,
    ControlDownload,
    ControlOpen,
    Accounts,
    AltLogout,
    AltRemoveLocal,
    FolderSettings,
    NewFolder,
    EditFolder,
    ShareFolder,
    DeleteFolder,
    ShiftUp,
    ShiftDown,
}

impl TestKey {
    pub(super) fn event(self) -> Event {
        let (code, modifiers) = match self {
            Self::Enter => (KeyCode::Enter, KeyModifiers::NONE),
            Self::Escape => (KeyCode::Esc, KeyModifiers::NONE),
            Self::Up => (KeyCode::Up, KeyModifiers::NONE),
            Self::Down => (KeyCode::Down, KeyModifiers::NONE),
            Self::Left => (KeyCode::Left, KeyModifiers::NONE),
            Self::Right => (KeyCode::Right, KeyModifiers::NONE),
            Self::AltUp => (KeyCode::Up, KeyModifiers::ALT),
            Self::AltDown => (KeyCode::Down, KeyModifiers::ALT),
            Self::AltLeft => (KeyCode::Left, KeyModifiers::ALT),
            Self::AltRight => (KeyCode::Right, KeyModifiers::ALT),
            Self::AltEdit => (KeyCode::Char('e'), KeyModifiers::ALT),
            Self::AltDelete => (KeyCode::Char('d'), KeyModifiers::ALT),
            Self::AltForward => (KeyCode::Char('f'), KeyModifiers::ALT),
            Self::AltReact => (KeyCode::Char('r'), KeyModifiers::ALT),
            Self::AltActions => (KeyCode::Char('a'), KeyModifiers::ALT),
            Self::SuperPaste => (KeyCode::Char('v'), KeyModifiers::SUPER),
            Self::ControlPaste => (KeyCode::Char('v'), KeyModifiers::CONTROL),
            Self::AltPin => (KeyCode::Char('p'), KeyModifiers::ALT),
            Self::Pinned => (KeyCode::Char('p'), KeyModifiers::NONE),
            Self::ControlPoll => (KeyCode::Char('p'), KeyModifiers::CONTROL),
            Self::ControlMedia => (KeyCode::Char('m'), KeyModifiers::CONTROL),
            Self::ControlScheduled => (KeyCode::Char('g'), KeyModifiers::CONTROL),
            Self::Reschedule => (KeyCode::Char('r'), KeyModifiers::NONE),
            Self::Vote => (KeyCode::Char('v'), KeyModifiers::NONE),
            Self::Space => (KeyCode::Char(' '), KeyModifiers::NONE),
            Self::ShiftEnter => (KeyCode::Enter, KeyModifiers::SHIFT),
            Self::Backspace => (KeyCode::Backspace, KeyModifiers::NONE),
            Self::AltReconnect => (KeyCode::Char('r'), KeyModifiers::ALT),
            Self::ControlReply => (KeyCode::Char('r'), KeyModifiers::CONTROL),
            Self::ControlThread => (KeyCode::Char('t'), KeyModifiers::CONTROL),
            Self::ControlLink => (KeyCode::Char('l'), KeyModifiers::CONTROL),
            Self::ControlDownload => (KeyCode::Char('d'), KeyModifiers::CONTROL),
            Self::ControlOpen => (KeyCode::Char('o'), KeyModifiers::CONTROL),
            Self::Accounts => (KeyCode::Char('a'), KeyModifiers::NONE),
            Self::AltLogout => (KeyCode::Char('l'), KeyModifiers::ALT),
            Self::AltRemoveLocal => (KeyCode::Char('d'), KeyModifiers::ALT),
            Self::FolderSettings => (KeyCode::Char('f'), KeyModifiers::NONE),
            Self::NewFolder => (KeyCode::Char('n'), KeyModifiers::NONE),
            Self::EditFolder => (KeyCode::Char('e'), KeyModifiers::NONE),
            Self::ShareFolder => (KeyCode::Char('s'), KeyModifiers::NONE),
            Self::DeleteFolder => (KeyCode::Char('d'), KeyModifiers::NONE),
            Self::ShiftUp => (KeyCode::Up, KeyModifiers::SHIFT),
            Self::ShiftDown => (KeyCode::Down, KeyModifiers::SHIFT),
        };
        Event::Key(KeyEvent::new_with_kind(
            code,
            modifiers,
            KeyEventKind::Press,
        ))
    }
}

impl TestSystem {
    /// Chooses one visible context action through the production keymap.
    pub fn choose_action(&mut self, label: &str) -> Result<()> {
        match self.application.view().focus {
            Focus::Transcript => self.type_text("a")?,
            Focus::Composer => self.press(key::ALT_ACTIONS)?,
            focus => {
                return Err(Error::Expectation {
                    expectation: format!("context actions are available from {focus:?}"),
                    actual: "Chat list or Search has focus".to_owned(),
                    artifact: self.trace.borrow().persist(),
                });
            }
        }
        let rows = self.screen().rows();
        let Some(title) = rows.iter().position(|row| row.contains(" Actions")) else {
            return Err(Error::Expectation {
                expectation: "a context-actions popup is visible".to_owned(),
                actual: rows.join("\n"),
                artifact: self.trace.borrow().persist(),
            });
        };
        let Some(target) = rows
            .iter()
            .skip(title + 2)
            .position(|row| row.contains(label))
        else {
            return Err(Error::Expectation {
                expectation: format!("context action {label:?} is visible"),
                actual: rows.join("\n"),
                artifact: self.trace.borrow().persist(),
            });
        };
        for _ in 0..target {
            self.press(key::DOWN)?;
        }
        self.press(key::ENTER)
    }
}
