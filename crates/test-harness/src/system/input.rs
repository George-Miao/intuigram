//! Named terminal inputs used by behavior scenarios.

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

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
    pub const ALT_DELETE: TestKey = TestKey::AltDelete;
    pub const ALT_FORWARD: TestKey = TestKey::AltForward;
    pub const ALT_REACT: TestKey = TestKey::AltReact;
    pub const ALT_PIN: TestKey = TestKey::AltPin;
    pub const PINNED: TestKey = TestKey::Pinned;
    pub const CTRL_POLL: TestKey = TestKey::ControlPoll;
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
    AltEdit,
    AltDelete,
    AltForward,
    AltReact,
    AltPin,
    Pinned,
    ControlPoll,
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
            Self::AltEdit => (KeyCode::Char('e'), KeyModifiers::ALT),
            Self::AltDelete => (KeyCode::Char('d'), KeyModifiers::ALT),
            Self::AltForward => (KeyCode::Char('f'), KeyModifiers::ALT),
            Self::AltReact => (KeyCode::Char('r'), KeyModifiers::ALT),
            Self::AltPin => (KeyCode::Char('p'), KeyModifiers::ALT),
            Self::Pinned => (KeyCode::Char('p'), KeyModifiers::NONE),
            Self::ControlPoll => (KeyCode::Char('p'), KeyModifiers::CONTROL),
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
