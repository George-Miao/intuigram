//! Named terminal inputs used by behavior scenarios.

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

pub mod key {
    use super::TestKey;

    pub const ENTER: TestKey = TestKey::Enter;
    pub const ESCAPE: TestKey = TestKey::Escape;
    pub const DOWN: TestKey = TestKey::Down;
    pub const ALT_UP: TestKey = TestKey::AltUp;
    pub const ALT_EDIT: TestKey = TestKey::AltEdit;
    pub const ALT_DELETE: TestKey = TestKey::AltDelete;
    pub const ALT_FORWARD: TestKey = TestKey::AltForward;
    pub const ALT_REACT: TestKey = TestKey::AltReact;
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
}

#[derive(Clone, Copy, Debug)]
pub enum TestKey {
    Enter,
    Escape,
    Down,
    AltUp,
    AltEdit,
    AltDelete,
    AltForward,
    AltReact,
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
}

impl TestKey {
    pub(super) fn event(self) -> Event {
        let (code, modifiers) = match self {
            Self::Enter => (KeyCode::Enter, KeyModifiers::NONE),
            Self::Escape => (KeyCode::Esc, KeyModifiers::NONE),
            Self::Down => (KeyCode::Down, KeyModifiers::NONE),
            Self::AltUp => (KeyCode::Up, KeyModifiers::ALT),
            Self::AltEdit => (KeyCode::Char('e'), KeyModifiers::ALT),
            Self::AltDelete => (KeyCode::Char('d'), KeyModifiers::ALT),
            Self::AltForward => (KeyCode::Char('f'), KeyModifiers::ALT),
            Self::AltReact => (KeyCode::Char('r'), KeyModifiers::ALT),
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
        };
        Event::Key(KeyEvent::new_with_kind(
            code,
            modifiers,
            KeyEventKind::Press,
        ))
    }
}
