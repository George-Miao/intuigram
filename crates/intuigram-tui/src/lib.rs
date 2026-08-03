//! Ratatui terminal adapter and shared effective keymap.

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
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, List, ListItem, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use snafu::{OptionExt, ResultExt, Snafu};

// OpenCode's Everforest light palette, kept local until themes become
// configurable.
const BACKGROUND: Color = Color::Rgb(253, 246, 227);
const SURFACE_BACKGROUND: Color = Color::Rgb(244, 240, 217);
const FOCUSED_SURFACE_BACKGROUND: Color = Color::Rgb(230, 226, 204);
const CHROME_BACKGROUND: Color = Color::Rgb(239, 235, 212);
const TEXT: Color = Color::Rgb(92, 106, 114);
const MUTED_TEXT: Color = Color::Rgb(130, 145, 129);
const PRIMARY: Color = Color::Rgb(141, 161, 1);
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

impl KeyChord {
    /// Creates an unmodified key.
    #[must_use]
    pub const fn plain(key: Key) -> Self {
        Self {
            key,
            control: false,
            shift: false,
            alt: false,
        }
    }

    /// Creates a Control-modified key.
    #[must_use]
    pub const fn control(key: Key) -> Self {
        Self {
            key,
            control: true,
            shift: false,
            alt: false,
        }
    }

    /// Creates an Alt-modified key.
    #[must_use]
    pub const fn alt(key: Key) -> Self {
        Self {
            key,
            control: false,
            shift: false,
            alt: true,
        }
    }

    /// Creates a Shift-modified key.
    #[must_use]
    pub const fn shift(key: Key) -> Self {
        Self {
            key,
            control: false,
            shift: true,
            alt: false,
        }
    }

    /// Formats the chord exactly as the terminal UI displays it.
    #[must_use]
    pub fn label(self) -> String {
        let mut label = String::new();
        if self.control {
            label.push_str("Ctrl+");
        }
        if self.alt {
            label.push_str("Alt+");
        }
        if self.shift {
            label.push_str("Shift+");
        }
        label.push_str(match self.key {
            Key::Char(character) => return format!("{label}{}", character.to_ascii_uppercase()),
            Key::Up => "Up",
            Key::Down => "Down",
            Key::Left => "Left",
            Key::Right => "Right",
            Key::Home => "Home",
            Key::End => "End",
            Key::Enter => "Enter",
            Key::Escape => "Esc",
        });
        label
    }
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

/// Event produced by the terminal adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UiEvent {
    /// Application intent resolved from a key or paste event.
    Intent(Intent),
    /// Terminal dimensions changed and the view should be redrawn.
    Redraw,
}

/// Failure while operating the terminal UI.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    /// Raw terminal mode could not be enabled.
    #[snafu(display("failed to enable terminal raw mode"))]
    EnableRawMode {
        /// Underlying terminal failure.
        source: io::Error,
    },

    /// The alternate screen could not be entered.
    #[snafu(display("failed to enter terminal alternate screen"))]
    EnterScreen {
        /// Underlying terminal failure.
        source: io::Error,
    },

    /// Enhanced keyboard reporting could not be enabled.
    #[snafu(display("failed to enable unambiguous terminal keyboard reporting"))]
    EnableKeyboardReporting {
        /// Underlying terminal failure.
        source: io::Error,
    },

    /// The terminal could not be initialized.
    #[snafu(display("failed to initialize terminal renderer"))]
    InitializeTerminal {
        /// Underlying terminal failure.
        source: io::Error,
    },

    /// A frame could not be drawn.
    #[snafu(display("failed to draw terminal frame"))]
    Draw {
        /// Underlying terminal failure.
        source: io::Error,
    },

    /// A terminal event could not be read.
    #[snafu(display("failed to read terminal input"))]
    ReadEvent {
        /// Underlying terminal failure.
        source: io::Error,
    },

    /// The Compio terminal event stream could not be initialized.
    #[snafu(display("failed to initialize terminal input"))]
    InitializeEventStream {
        /// Underlying terminal-event adapter failure.
        source: compio_term::EventError,
    },

    /// The Compio terminal event stream failed.
    #[snafu(display("failed to receive terminal input"))]
    StreamEvent {
        /// Underlying terminal-event adapter failure.
        source: compio_term::EventError,
    },

    /// The terminal event source ended while the UI was active.
    #[snafu(display("terminal input closed"))]
    EventStreamClosed,

    /// A Telegram login URI could not be encoded as a QR symbol.
    #[snafu(display("failed to encode Telegram login QR code"))]
    EncodeQr {
        /// Underlying QR encoding failure.
        source: qrcode::types::QrError,
    },
}

/// Result returned by terminal operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// User action available from the QR-login screen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QrLoginAction {
    /// No relevant terminal input is waiting.
    None,

    /// Redraw after a terminal resize.
    Redraw,

    /// Fall back to phone-number authentication.
    PhoneLogin,

    /// Abort Intuigram startup.
    Cancel,
}

/// Temporary alternate-screen session used during Telegram QR login.
pub struct QrLoginUi {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl QrLoginUi {
    /// Enters raw mode and the alternate screen.
    pub fn enter() -> Result<Self> {
        Ok(Self {
            terminal: enter_terminal()?,
        })
    }

    /// Draws the current QR token and its remaining lifetime.
    pub fn draw(&mut self, uri: &str, expires_in: u64) -> Result<()> {
        let qr = qr_login_symbols(uri)?;
        self.terminal
            .draw(|frame| render_qr_login(frame, &qr, expires_in))
            .context(DrawSnafu)?;
        Ok(())
    }

    /// Polls for a QR-screen action without blocking the Telegram receive loop.
    pub fn poll_action(&self, timeout: Duration) -> Result<QrLoginAction> {
        if !event::poll(timeout).context(ReadEventSnafu)? {
            return Ok(QrLoginAction::None);
        }
        match event::read().context(ReadEventSnafu)? {
            Event::Resize(..) => Ok(QrLoginAction::Redraw),
            Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                match key.code {
                    CrosstermKey::Char('p' | 'P')
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                    {
                        Ok(QrLoginAction::PhoneLogin)
                    }
                    CrosstermKey::Esc => Ok(QrLoginAction::Cancel),
                    CrosstermKey::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        Ok(QrLoginAction::Cancel)
                    }
                    _ => Ok(QrLoginAction::None),
                }
            }
            _ => Ok(QrLoginAction::None),
        }
    }
}

impl Drop for QrLoginUi {
    fn drop(&mut self) {
        restore_terminal(&mut self.terminal);
    }
}

/// Active alternate-screen terminal session.
pub struct TerminalUi {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    keymap: EffectiveKeymap,
}

impl TerminalUi {
    /// Enters raw mode and the alternate screen.
    pub fn enter() -> Result<Self> {
        Ok(Self {
            terminal: enter_terminal()?,
            keymap: EffectiveKeymap::defaults(),
        })
    }

    /// Draws one immutable application view.
    pub fn draw(&mut self, view: &View) -> Result<()> {
        let keymap = &self.keymap;
        self.terminal
            .draw(|frame| render(frame, view, keymap))
            .context(DrawSnafu)?;
        Ok(())
    }

    /// Resolves a raw terminal event against the latest application view.
    #[must_use]
    pub fn resolve_event(&self, view: &View, event: Event) -> Option<UiEvent> {
        resolve_event(&self.keymap, view, event)
    }
}

/// Persistent Compio-driven terminal input source.
#[derive(Debug)]
pub struct TerminalEvents {
    events: EventStream,
}

impl TerminalEvents {
    /// Opens the controlling terminal event source on the active Compio
    /// runtime thread.
    pub fn new() -> Result<Self> {
        Ok(Self {
            events: EventStream::new().context(InitializeEventStreamSnafu)?,
        })
    }

    /// Waits for one raw terminal event without binding it to a stale view.
    pub async fn next_event(&mut self) -> Result<Event> {
        self.events
            .next()
            .await
            .context(EventStreamClosedSnafu)?
            .context(StreamEventSnafu)
    }

    /// Polls the persistent source in-place so callers can multiplex it
    /// without constructing and cancelling one-shot read futures.
    pub fn poll_next_event(&mut self, cx: &mut Context<'_>) -> Poll<Result<Event>> {
        match Pin::new(&mut self.events).poll_next(cx) {
            Poll::Ready(Some(event)) => Poll::Ready(event.context(StreamEventSnafu)),
            Poll::Ready(None) => Poll::Ready(EventStreamClosedSnafu.fail()),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Drop for TerminalUi {
    fn drop(&mut self) {
        restore_terminal(&mut self.terminal);
    }
}

fn enter_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode().context(EnableRawModeSnafu)?;
    let mut stdout = io::stdout();
    if let Err(source) = execute!(stdout, EnterAlternateScreen) {
        let _ = disable_raw_mode();
        return Err(Error::EnterScreen { source });
    }
    if let Err(source) = execute!(
        stdout,
        PushKeyboardEnhancementFlags(terminal_keyboard_flags())
    ) {
        let _ = execute!(stdout, LeaveAlternateScreen);
        let _ = disable_raw_mode();
        return Err(Error::EnableKeyboardReporting { source });
    }
    match Terminal::new(CrosstermBackend::new(stdout)) {
        Ok(terminal) => Ok(terminal),
        Err(source) => {
            let mut stdout = io::stdout();
            let _ = execute!(stdout, PopKeyboardEnhancementFlags, LeaveAlternateScreen);
            let _ = disable_raw_mode();
            Err(Error::InitializeTerminal { source })
        }
    }
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) {
    let _ = disable_raw_mode();
    let _ = execute!(
        terminal.backend_mut(),
        PopKeyboardEnhancementFlags,
        LeaveAlternateScreen
    );
    let _ = terminal.show_cursor();
}

const fn terminal_keyboard_flags() -> KeyboardEnhancementFlags {
    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        .union(KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES)
}

fn resolve_event(keymap: &EffectiveKeymap, view: &View, event: Event) -> Option<UiEvent> {
    match event {
        Event::Resize(..) => Some(UiEvent::Redraw),
        Event::Paste(text) => Some(UiEvent::Intent(Intent::Insert(text))),
        Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
            let chord = chord_from_crossterm(key.code, key.modifiers);
            if let Some(chord) = chord
                && let Some(action) = keymap.resolve(view, chord)
            {
                return Some(UiEvent::Intent(Intent::Action(action)));
            }
            match key.code {
                CrosstermKey::Char(character)
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    Some(UiEvent::Intent(Intent::Insert(character.to_string())))
                }
                CrosstermKey::Backspace => Some(UiEvent::Intent(Intent::Backspace)),
                _ => None,
            }
        }
        _ => None,
    }
}

struct QrLoginSymbols {
    dense: String,
    compact: String,
}

fn qr_login_symbols(uri: &str) -> Result<QrLoginSymbols> {
    let dense = QrCode::new(uri.as_bytes()).context(EncodeQrSnafu)?;
    let compact =
        QrCode::with_error_correction_level(uri.as_bytes(), EcLevel::L).context(EncodeQrSnafu)?;
    Ok(QrLoginSymbols {
        dense: dense
            .render::<Dense1x2>()
            .module_dimensions(1, 1)
            .quiet_zone(true)
            .build(),
        compact: render_braille_qr(&compact),
    })
}

fn render_braille_qr(code: &QrCode) -> String {
    const QUIET_ZONE: usize = 4;
    const BRAILLE: [[u8; 2]; 4] = [[0, 3], [1, 4], [2, 5], [6, 7]];

    let source_width = code.width();
    let width = source_width + QUIET_ZONE * 2;
    let colors = code.to_colors();
    let mut rendered = String::new();
    for cell_y in (0..width).step_by(4) {
        if cell_y > 0 {
            rendered.push('\n');
        }
        for cell_x in (0..width).step_by(2) {
            let mut dots = 0_u8;
            for (dy, row) in BRAILLE.iter().enumerate() {
                for (dx, bit) in row.iter().enumerate() {
                    let x = cell_x + dx;
                    let y = cell_y + dy;
                    if x >= QUIET_ZONE
                        && y >= QUIET_ZONE
                        && x < source_width + QUIET_ZONE
                        && y < source_width + QUIET_ZONE
                        && colors[(y - QUIET_ZONE) * source_width + (x - QUIET_ZONE)]
                            == QrColor::Dark
                    {
                        dots |= 1 << bit;
                    }
                }
            }
            rendered.push(char::from_u32(0x2800 + u32::from(dots)).expect("valid Braille cell"));
        }
    }
    rendered
}

fn render_qr_login(frame: &mut Frame<'_>, qr: &QrLoginSymbols, expires_in: u64) {
    let area = frame.area();
    let rows = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "Link Intuigram to Telegram",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from("Scan in Telegram: Settings → Devices → Link Desktop Device"),
        ])
        .alignment(Alignment::Center),
        rows[0],
    );

    let symbol = [&qr.dense, &qr.compact].into_iter().find(|symbol| {
        let width = symbol
            .lines()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0);
        symbol.lines().count() <= usize::from(rows[1].height) && width <= usize::from(rows[1].width)
    });
    if let Some(symbol) = symbol {
        let qr_height = u16::try_from(symbol.lines().count()).unwrap_or(u16::MAX);
        let qr_width = u16::try_from(
            symbol
                .lines()
                .map(|line| line.chars().count())
                .max()
                .unwrap_or(0),
        )
        .unwrap_or(u16::MAX);
        let qr_area = Rect {
            x: rows[1].x + rows[1].width.saturating_sub(qr_width) / 2,
            y: rows[1].y + rows[1].height.saturating_sub(qr_height) / 2,
            width: qr_width,
            height: qr_height,
        };
        frame.render_widget(
            Paragraph::new(symbol.as_str())
                .style(Style::default().fg(Color::Black).bg(Color::White)),
            qr_area,
        );
    } else {
        frame.render_widget(
            Paragraph::new("Terminal is too small to display a scannable QR code")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::Yellow)),
            rows[1],
        );
    }

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "P",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Phone login  "),
            Span::styled(
                "Esc",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Cancel"),
        ])),
        rows[2],
    );
    frame.render_widget(
        Paragraph::new(format!(
            " waiting for scan · refreshes automatically · expires in {expires_in}s"
        ))
        .style(Style::default().fg(Color::Black).bg(Color::DarkGray)),
        rows[3],
    );
}

fn chord_from_crossterm(code: CrosstermKey, modifiers: KeyModifiers) -> Option<KeyChord> {
    let key = match code {
        CrosstermKey::Char(character) => Key::Char(character.to_ascii_lowercase()),
        CrosstermKey::Up => Key::Up,
        CrosstermKey::Down => Key::Down,
        CrosstermKey::Left => Key::Left,
        CrosstermKey::Right => Key::Right,
        CrosstermKey::Home => Key::Home,
        CrosstermKey::End => Key::End,
        CrosstermKey::Enter => Key::Enter,
        CrosstermKey::Esc => Key::Escape,
        _ => return None,
    };
    Some(KeyChord {
        key,
        control: modifiers.contains(KeyModifiers::CONTROL),
        shift: modifiers.contains(KeyModifiers::SHIFT),
        alt: modifiers.contains(KeyModifiers::ALT),
    })
}

fn render(frame: &mut Frame<'_>, view: &View, keymap: &EffectiveKeymap) {
    let area = frame.area();
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new("").style(Style::default().fg(TEXT).bg(BACKGROUND)),
        area,
    );
    let rows = Layout::vertical([
        Constraint::Min(5),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);
    render_main(frame, rows[0], view);
    render_folders(frame, rows[1], view);
    render_actions(frame, rows[2], view, keymap);
    render_status(frame, rows[3], view);
    if view.help_open {
        render_help(frame, area, view, keymap);
    } else if view.folder_picker.is_some() {
        render_folder_picker(frame, area, view);
    }
}

fn render_main(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let columns = if area.width >= 120 {
        Layout::horizontal([
            Constraint::Length(32),
            Constraint::Length(1),
            Constraint::Min(48),
        ])
        .split(area)
    } else {
        Layout::horizontal([
            Constraint::Length(30),
            Constraint::Length(1),
            Constraint::Min(40),
        ])
        .split(area)
    };
    render_chats(frame, columns[0], view);
    render_active_chat(frame, columns[2], view);
}

fn render_chats(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let focused = view.focus == Focus::Chats;
    let rows = Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).split(area);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Chats", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("  {}", view.account_name),
                Style::default().fg(MUTED_TEXT),
            ),
        ]))
        .style(surface_style(focused)),
        rows[0],
    );
    let visible_items = usize::from(rows[1].height) / 2;
    let range = anchored_window(view.chats.len(), view.active_chat, visible_items, false);
    let items = view.chats[range.clone()]
        .iter()
        .enumerate()
        .map(|(offset, chat)| {
            let index = range.start + offset;
            let marker = if chat.pinned { "●" } else { " " };
            let unread = if chat.unread > 0 {
                format!(" {}", chat.unread)
            } else {
                String::new()
            };
            let selected = view.active_chat == Some(index);
            ListItem::new(vec![
                Line::from(vec![
                    selection_rule(selected),
                    Span::styled(
                        format!("{marker} {}", chat.title),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(unread, Style::default().fg(PRIMARY)),
                ]),
                Line::from(vec![
                    selection_rule(selected),
                    Span::styled(chat.preview.clone(), Style::default().fg(MUTED_TEXT)),
                ]),
            ])
        });
    frame.render_widget(List::new(items).style(surface_style(focused)), rows[1]);
}

fn render_active_chat(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let composer_height = if view.active_chat.is_some() { 3 } else { 0 };
    let rows = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(composer_height),
    ])
    .split(area);
    let header = view
        .active_chat
        .and_then(|index| view.chats.get(index))
        .map_or_else(
            || {
                Line::from(Span::styled(
                    "No active Chat",
                    Style::default().fg(MUTED_TEXT),
                ))
            },
            |chat| {
                Line::from(vec![
                    Span::styled(
                        chat.title.clone(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        if let Some(root) = view.active_thread {
                            format!("  Thread {}", root.0)
                        } else if chat.unread > 0 {
                            format!("  {} unread", chat.unread)
                        } else {
                            "  up to date".to_owned()
                        },
                        Style::default().fg(MUTED_TEXT),
                    ),
                ])
            },
        );
    let active_message = view
        .active_message
        .and_then(|index| view.messages.get(index))
        .map_or_else(
            || Line::from(""),
            |message| {
                Line::from(vec![
                    selection_rule(true),
                    Span::styled(
                        "Active message",
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" · {} · {}", message.sender, message.timestamp),
                        Style::default().fg(MUTED_TEXT),
                    ),
                ])
            },
        );
    frame.render_widget(
        Paragraph::new(vec![header, active_message])
            .style(surface_style(view.focus == Focus::Transcript)),
        rows[0],
    );
    render_transcript(frame, rows[1], view, view.focus == Focus::Transcript);
    if composer_height > 0 {
        render_composer(frame, rows[2], view);
    }
}

fn render_transcript(frame: &mut Frame<'_>, area: Rect, view: &View, focused: bool) {
    let visible_items = usize::from(area.height) / 2;
    let range = anchored_window(
        view.messages.len(),
        view.active_message.or(view.transcript_anchor),
        visible_items,
        true,
    );
    let items = view.messages[range.clone()]
        .iter()
        .enumerate()
        .map(|(offset, message)| {
            let index = range.start + offset;
            let selected = view.active_message == Some(index);
            let direction = match message.direction {
                MessageDirection::Incoming => "←",
                MessageDirection::Outgoing => "→",
            };
            let delivery = match message.delivery {
                DeliveryState::Pending => "…",
                DeliveryState::Sent => "✓",
                DeliveryState::Read => "✓✓",
                DeliveryState::Failed => "!",
            };
            let reply = message
                .reply_to
                .map_or_else(String::new, |id| format!(" ↩{}", id.0));
            let forwarded = message
                .details
                .forwarded_from
                .as_ref()
                .map_or_else(String::new, |source| format!(" · forwarded from {source}"));
            let metadata = message_metadata(message);
            let header = Line::from(vec![
                selection_rule(selected),
                Span::styled(
                    format!("{direction} {}", message.sender),
                    Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
                ),
                Span::styled(reply, Style::default().fg(MUTED_TEXT)),
                Span::styled(forwarded, Style::default().fg(MUTED_TEXT)),
                Span::raw("  "),
                Span::styled(
                    format!("{} {delivery}", message.timestamp),
                    Style::default().fg(MUTED_TEXT),
                ),
            ]);
            let mut body = vec![selection_rule(selected)];
            body.extend(render_rich_text(message));
            body.push(Span::styled(metadata, Style::default().fg(MUTED_TEXT)));
            let mut lines = vec![header, Line::from(body)];
            if let Some(media) = &message.details.media {
                lines.push(Line::from(vec![
                    selection_rule(selected),
                    Span::styled(
                        format!("[{}]", media.title),
                        Style::default().fg(SECONDARY).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("  {}", media.description),
                        Style::default().fg(MUTED_TEXT),
                    ),
                ]));
            }
            ListItem::new(lines)
        });
    frame.render_widget(List::new(items).style(surface_style(focused)), area);
}

fn message_metadata(message: &MessageView) -> String {
    let mut parts = Vec::new();
    if message.details.edited {
        parts.push("edited".to_owned());
    }
    if message.details.pinned {
        parts.push("pinned".to_owned());
    }
    if let Some(views) = message.details.views {
        parts.push(format!("{views} views"));
    }
    if let Some(forwards) = message.details.forwards {
        parts.push(format!("{forwards} forwards"));
    }
    if let Some(replies) = message.details.replies {
        parts.push(format!("{replies} replies"));
    }
    parts.extend(
        message
            .details
            .reactions
            .iter()
            .map(|reaction| format!("{} {}", reaction.label, reaction.count)),
    );
    if parts.is_empty() {
        String::new()
    } else {
        format!("  · {}", parts.join(" · "))
    }
}

fn render_rich_text(message: &MessageView) -> Vec<Span<'static>> {
    if message.details.entities.is_empty() {
        return vec![Span::raw(message.body.clone())];
    }
    let mut result = Vec::new();
    let mut utf16_offset = 0;
    for character in message.body.chars() {
        let character_length = character.len_utf16();
        let mut style = Style::default();
        for entity in &message.details.entities {
            let entity_end = entity.offset.saturating_add(entity.length);
            if utf16_offset < entity_end && utf16_offset + character_length > entity.offset {
                style = match &entity.kind {
                    TextEntityKind::Bold => style.add_modifier(Modifier::BOLD),
                    TextEntityKind::Italic => style.add_modifier(Modifier::ITALIC),
                    TextEntityKind::Underline => style.add_modifier(Modifier::UNDERLINED),
                    TextEntityKind::Strike => style.add_modifier(Modifier::CROSSED_OUT),
                    TextEntityKind::Code | TextEntityKind::Pre { .. } => {
                        style.fg(SECONDARY).bg(SURFACE_BACKGROUND)
                    }
                    TextEntityKind::Spoiler => style.fg(MUTED_TEXT),
                    TextEntityKind::Url | TextEntityKind::TextUrl { .. } => {
                        style.fg(PRIMARY).add_modifier(Modifier::UNDERLINED)
                    }
                    TextEntityKind::Semantic | TextEntityKind::CustomEmoji { .. } => {
                        style.fg(PRIMARY)
                    }
                };
            }
        }
        result.push(Span::styled(character.to_string(), style));
        utf16_offset += character_length;
    }
    result
}

fn render_composer(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let focused = view.focus == Focus::Composer;
    let composer_label = view.composer.reply_to.map_or_else(
        || "Draft".to_owned(),
        |message| format!("Reply to {}", message.0),
    );
    let composer_label = if view.composer.attachments.is_empty() {
        composer_label
    } else {
        format!(
            "{composer_label} · {} attachment(s)",
            view.composer.attachments.len()
        )
    };
    let draft_content = if view.composer.text.is_empty() {
        Span::styled("Type or paste a message…", Style::default().fg(MUTED_TEXT))
    } else {
        Span::raw(view.composer.text.clone())
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                interaction_rule(focused),
                Span::styled(composer_label, Style::default().fg(MUTED_TEXT)),
            ]),
            Line::from(vec![interaction_rule(focused), draft_content]),
        ])
        .style(surface_style(focused))
        .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_folders(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let spans = view.folders.iter().enumerate().flat_map(|(index, folder)| {
        let active = index == view.active_folder;
        let style = if active {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(MUTED_TEXT)
        };
        let unread = if folder.unread > 0 {
            format!(" {}", folder.unread)
        } else {
            String::new()
        };
        [
            selection_rule(active),
            Span::styled(format!("{}{unread}", folder.title), style),
            Span::raw(" "),
        ]
    });
    frame.render_widget(
        Paragraph::new(Line::from(spans.collect::<Vec<_>>())).style(surface_style(false)),
        area,
    );
}

fn render_actions(frame: &mut Frame<'_>, area: Rect, view: &View, keymap: &EffectiveKeymap) {
    let mut spans = Vec::new();
    for binding in keymap.action_bar(view) {
        spans.push(Span::styled(
            binding.key.label(),
            Style::default().fg(PRIMARY).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(format!(" {}  ", binding.label)));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(surface_style(false)),
        area,
    );
}

fn render_status(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let connection = match view.connection {
        ConnectionState::Connected => "connected",
        ConnectionState::Connecting => "connecting",
        ConnectionState::ReconnectCooldown => "reconnect cooldown",
    };
    let mut status = format!("{} · {:?} · {connection}", view.account_name, view.focus);
    if let Some(search) = &view.search {
        write!(status, " · {:?} search: {}", search.scope, search.query)
            .expect("writing to a String cannot fail");
    }
    if view.has_newer_messages {
        status.push_str(" · new messages ↓");
    }
    if let Some(notice) = &view.notice {
        write!(status, " · {notice}").expect("writing to a String cannot fail");
    }
    let style = if view.focus == Focus::Search {
        surface_style(true)
    } else {
        Style::default().fg(MUTED_TEXT).bg(CHROME_BACKGROUND)
    };
    frame.render_widget(Paragraph::new(status).style(style), area);
}

fn render_help(frame: &mut Frame<'_>, area: Rect, view: &View, keymap: &EffectiveKeymap) {
    let popup = centered_rect(70, 75, area);
    let lines = std::iter::once(Line::from(Span::styled(
        "Context Help",
        Style::default().add_modifier(Modifier::BOLD),
    )))
    .chain(std::iter::once(Line::from("")))
    .chain(keymap.help(view).map(|binding| {
        Line::from(vec![
            Span::styled(
                format!("{:>12}", binding.key.label()),
                Style::default().fg(PRIMARY),
            ),
            Span::raw(format!("  {}", binding.label)),
        ])
    }));
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines.collect::<Vec<_>>())
            .style(surface_style(true))
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn render_folder_picker(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let popup = centered_rect(52, 60, area);
    let memberships = view
        .active_chat
        .and_then(|index| view.chats.get(index))
        .map(|chat| chat.folders.as_slice())
        .unwrap_or_default();
    let lines = std::iter::once(Line::from(Span::styled(
        "Folder membership",
        Style::default().add_modifier(Modifier::BOLD),
    )))
    .chain(std::iter::once(Line::from(Span::styled(
        "Choose a Folder to add or remove this Chat",
        Style::default().fg(MUTED_TEXT),
    ))))
    .chain(std::iter::once(Line::from("")))
    .chain(
        view.folders
            .iter()
            .enumerate()
            .skip(1)
            .map(|(index, folder)| {
                let selected = view.folder_picker == Some(index);
                let marker = if memberships.contains(&folder.id) {
                    "[x]"
                } else {
                    "[ ]"
                };
                Line::from(vec![
                    selection_rule(selected),
                    Span::styled(
                        format!("{marker} {}", folder.title),
                        Style::default().fg(if selected { PRIMARY } else { TEXT }),
                    ),
                ])
            }),
    );
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines.collect::<Vec<_>>())
            .style(surface_style(true))
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn selection_rule(selected: bool) -> Span<'static> {
    if selected {
        Span::styled("│ ", Style::default().fg(PRIMARY))
    } else {
        Span::raw("  ")
    }
}

fn interaction_rule(active: bool) -> Span<'static> {
    if active {
        Span::styled("│ ", Style::default().fg(PRIMARY))
    } else {
        Span::styled("│ ", Style::default().fg(MUTED_TEXT))
    }
}

fn surface_style(focused: bool) -> Style {
    Style::default().fg(TEXT).bg(if focused {
        FOCUSED_SURFACE_BACKGROUND
    } else {
        SURFACE_BACKGROUND
    })
}

fn anchored_window(
    length: usize,
    active: Option<usize>,
    visible_items: usize,
    default_to_end: bool,
) -> std::ops::Range<usize> {
    let visible_items = visible_items.max(1).min(length);
    let active = active
        .map(|index| index.min(length.saturating_sub(1)))
        .or_else(|| default_to_end.then(|| length.saturating_sub(1)))
        .unwrap_or(0);
    let anchor = visible_items / 3;
    let start = active
        .saturating_sub(anchor)
        .min(length.saturating_sub(visible_items));
    start..start + visible_items
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1])[1]
}

#[cfg(test)]
mod tests {
    use crossterm::event::{
        Event, KeyCode as CrosstermKey, KeyEvent, KeyEventKind, KeyModifiers,
        KeyboardEnhancementFlags,
    };
    use intuigram_app::{
        Action, ChatId, ChatKind, ChatView, ComposerView, ConnectionState, DeliveryState, Focus,
        FolderView, MediaCard, MediaKind, MessageDetails, MessageDirection, MessageId, MessageView,
        SearchView, View,
    };
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;

    use super::{
        EffectiveKeymap, Key, KeyChord, UiEvent, chord_from_crossterm, qr_login_symbols, render,
        resolve_event, terminal_keyboard_flags,
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
            folders: Vec::new(),
            active_folder: 0,
            chats: Vec::new(),
            active_chat: None,
            messages: Vec::new(),
            active_message: None,
            active_thread: None,
            transcript_anchor: None,
            focus: Focus::Chats,
            composer: ComposerView::default(),
            search: None::<SearchView>,
            has_newer_messages: false,
            help_open: false,
            folder_picker: None,
            notice: None,
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

        let composer = view(vec![Action::Send, Action::Newline]);
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
            keymap
                .action_bar(&composer)
                .find(|binding| binding.action == Action::Newline)
                .map(|binding| binding.key),
            Some(KeyChord::shift(Key::Enter))
        );
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
            keymap.resolve(&chat_list, KeyChord::alt(Key::Left)),
            Some(Action::PreviousFolder)
        );
        assert_eq!(
            keymap.resolve(&chat_list, KeyChord::alt(Key::Right)),
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
        assert_eq!(
            keymap.resolve(&composer, KeyChord::alt(Key::Up)),
            Some(Action::TargetPreviousMessage)
        );
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
    fn terminal_keyboard_protocol_disambiguates_modified_enter() {
        let flags = terminal_keyboard_flags();

        assert!(flags.contains(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES));
        assert!(flags.contains(KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES));
    }

    #[test]
    fn folder_membership_overlay_shows_selection_and_current_membership() {
        let mut view = view(vec![
            Action::MoveUp,
            Action::MoveDown,
            Action::ToggleFolderMembership,
            Action::Cancel,
        ]);
        view.folders = vec![
            FolderView {
                id: 0,
                title: "All".to_owned(),
                unread: 0,
            },
            FolderView {
                id: 2,
                title: "Work".to_owned(),
                unread: 4,
            },
        ];
        view.chats = vec![ChatView {
            id: ChatId(10),
            title: "Intuigram".to_owned(),
            preview: String::new(),
            unread: 0,
            pinned: false,
            kind: ChatKind::Private,
            folders: vec![0, 2],
        }];
        view.active_chat = Some(0);
        view.folder_picker = Some(1);

        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
        terminal
            .draw(|frame| render(frame, &view, &EffectiveKeymap::defaults()))
            .expect("view should render");
        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("Folder membership"));
        assert!(rendered.contains("[x] Work"));
    }

    #[test]
    fn chat_list_scroll_keeps_the_active_chat_near_one_third_height() {
        let mut view = view(Vec::new());
        view.chats = (0..14)
            .map(|index| ChatView {
                id: ChatId(index),
                title: format!("Chat {index}"),
                preview: format!("Preview {index}"),
                unread: 0,
                pinned: false,
                kind: ChatKind::Private,
                folders: vec![0],
            })
            .collect();
        view.active_chat = Some(8);

        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
        terminal
            .draw(|frame| render(frame, &view, &EffectiveKeymap::defaults()))
            .expect("view should render");
        let buffer = terminal.backend().buffer();
        let rendered: String = buffer.content.iter().map(|cell| cell.symbol()).collect();

        assert_eq!(buffer[(0, 8)].symbol(), "│");
        assert!(rendered.contains("Chat 8"));
        assert!(!rendered.contains("Chat 0"));
    }

    #[test]
    fn transcript_scroll_keeps_the_active_message_visible() {
        let mut view = view(Vec::new());
        view.chats = vec![ChatView {
            id: ChatId(10),
            title: "Intuigram".to_owned(),
            preview: String::new(),
            unread: 0,
            pinned: false,
            kind: ChatKind::Private,
            folders: vec![0],
        }];
        view.active_chat = Some(0);
        view.messages = (0..20)
            .map(|index| MessageView {
                id: MessageId(index),
                sender: "Lin".to_owned(),
                body: format!("Message {index}"),
                timestamp: "12:00".to_owned(),
                direction: MessageDirection::Incoming,
                delivery: DeliveryState::Read,
                reply_to: None,
                details: MessageDetails::default(),
            })
            .collect();
        view.active_message = Some(10);
        view.focus = Focus::Transcript;

        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
        terminal
            .draw(|frame| render(frame, &view, &EffectiveKeymap::defaults()))
            .expect("view should render");
        let buffer = terminal.backend().buffer();

        assert_eq!(buffer[(31, 6)].symbol(), "│");
        assert_eq!(buffer[(33, 7)].symbol(), "M");
    }

    #[test]
    fn transcript_scroll_preserves_an_inactive_anchor() {
        let mut view = view(Vec::new());
        view.chats = vec![ChatView {
            id: ChatId(10),
            title: "Intuigram".to_owned(),
            preview: String::new(),
            unread: 0,
            pinned: false,
            kind: ChatKind::Private,
            folders: vec![0],
        }];
        view.active_chat = Some(0);
        view.messages = (0..20)
            .map(|index| MessageView {
                id: MessageId(index),
                sender: "Lin".to_owned(),
                body: format!("Message {index}"),
                timestamp: "12:00".to_owned(),
                direction: MessageDirection::Incoming,
                delivery: DeliveryState::Read,
                reply_to: None,
                details: MessageDetails::default(),
            })
            .collect();
        view.transcript_anchor = Some(10);
        view.focus = Focus::Composer;

        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
        terminal
            .draw(|frame| render(frame, &view, &EffectiveKeymap::defaults()))
            .expect("view should render");
        let buffer = terminal.backend().buffer();

        assert_eq!(buffer[(31, 6)].symbol(), " ");
        assert_eq!(buffer[(33, 7)].symbol(), "M");
    }

    #[test]
    fn transcript_keeps_media_card_fallback_visible_beside_a_caption() {
        let mut view = view(Vec::new());
        view.chats = vec![ChatView {
            id: ChatId(10),
            title: "Intuigram".to_owned(),
            preview: String::new(),
            unread: 0,
            pinned: false,
            kind: ChatKind::Private,
            folders: vec![0],
        }];
        view.active_chat = Some(0);
        view.messages = vec![MessageView {
            id: MessageId(1),
            sender: "Lin".to_owned(),
            body: "caption".to_owned(),
            timestamp: "12:00".to_owned(),
            direction: MessageDirection::Incoming,
            delivery: DeliveryState::Read,
            reply_to: None,
            details: MessageDetails {
                media: Some(MediaCard {
                    kind: MediaKind::Unsupported,
                    title: "Unsupported Content".to_owned(),
                    description: "constructor retained".to_owned(),
                    remote_id: None,
                }),
                ..MessageDetails::default()
            },
        }];

        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
        terminal
            .draw(|frame| render(frame, &view, &EffectiveKeymap::defaults()))
            .expect("view should render");
        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("Unsupported Content"));
        assert!(rendered.contains("constructor retained"));
    }

    #[test]
    fn everforest_light_palette_is_used_for_the_terminal_surface() {
        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
        terminal
            .draw(|frame| render(frame, &view(Vec::new()), &EffectiveKeymap::defaults()))
            .expect("view should render");
        let buffer = terminal.backend().buffer();

        assert_eq!(buffer[(30, 5)].bg, Color::Rgb(253, 246, 227));
        assert_eq!(buffer[(5, 5)].bg, Color::Rgb(230, 226, 204));
        assert_eq!(buffer[(5, 5)].fg, Color::Rgb(92, 106, 114));
    }

    #[test]
    fn redrawing_shorter_chat_text_clears_the_previous_frame() {
        let mut view = view(Vec::new());
        view.chats = vec![ChatView {
            id: ChatId(10),
            title: "A title with stale trailing characters".to_owned(),
            preview: "A preview with stale trailing characters".to_owned(),
            unread: 0,
            pinned: false,
            kind: ChatKind::Private,
            folders: vec![0],
        }];
        view.active_chat = Some(0);
        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
        terminal
            .draw(|frame| render(frame, &view, &EffectiveKeymap::defaults()))
            .expect("long view should render");

        view.chats[0].title = "X".to_owned();
        view.chats[0].preview.clear();
        terminal
            .draw(|frame| render(frame, &view, &EffectiveKeymap::defaults()))
            .expect("short view should render");
        let buffer = terminal.backend().buffer();

        assert!((5..30).all(|x| buffer[(x, 2)].symbol() == " "));
        assert!((2..30).all(|x| buffer[(x, 3)].symbol() == " "));
    }

    #[test]
    fn side_by_side_render_separates_sections_and_highlights_the_interaction_target() {
        let mut view = view(vec![
            Action::Send,
            Action::Cancel,
            Action::TargetPreviousMessage,
        ]);
        view.account_name = "Ada".to_owned();
        view.folders = vec![FolderView {
            id: 0,
            title: "All".to_owned(),
            unread: 1,
        }];
        view.chats = vec![ChatView {
            id: ChatId(10),
            title: "Intuigram".to_owned(),
            preview: "daily driver".to_owned(),
            unread: 1,
            pinned: true,
            kind: ChatKind::Supergroup,
            folders: vec![0],
        }];
        view.active_chat = Some(0);
        view.messages = vec![MessageView {
            id: MessageId(1),
            sender: "Lin".to_owned(),
            body: "hello".to_owned(),
            timestamp: "12:00".to_owned(),
            direction: MessageDirection::Incoming,
            delivery: DeliveryState::Read,
            reply_to: Some(MessageId(0)),
            details: MessageDetails::default(),
        }];
        view.active_message = Some(0);
        view.focus = Focus::Composer;

        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
        terminal
            .draw(|frame| render(frame, &view, &EffectiveKeymap::defaults()))
            .expect("view should render");
        let buffer = terminal.backend().buffer();

        assert_eq!(buffer[(0, 2)].symbol(), "│");
        assert_eq!(buffer[(31, 2)].symbol(), "│");
        assert_eq!(buffer[(39, 2)].symbol(), "↩");
        assert_eq!(buffer[(39, 2)].fg, Color::Rgb(130, 145, 129));
        assert_eq!(buffer[(31, 18)].symbol(), "│");
        assert_eq!(buffer[(33, 18)].symbol(), "D");
        assert_eq!(buffer[(5, 5)].bg, Color::Rgb(244, 240, 217));
        assert_eq!(buffer[(40, 5)].bg, Color::Rgb(244, 240, 217));
        assert_eq!(buffer[(40, 18)].bg, Color::Rgb(230, 226, 204));
        assert_eq!(buffer[(30, 5)].bg, Color::Rgb(253, 246, 227));
        assert_eq!(buffer[(5, 21)].bg, Color::Rgb(244, 240, 217));
        assert_eq!(buffer[(5, 22)].bg, Color::Rgb(244, 240, 217));
        assert_eq!(buffer[(5, 23)].bg, Color::Rgb(239, 235, 212));
        assert!(
            buffer
                .content
                .iter()
                .all(|cell| { !matches!(cell.symbol(), "┌" | "┐" | "└" | "┘" | "─") })
        );

        view.focus = Focus::Chats;
        terminal
            .draw(|frame| render(frame, &view, &EffectiveKeymap::defaults()))
            .expect("Chat-list focus should render");
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(5, 5)].bg, Color::Rgb(230, 226, 204));
        assert_eq!(buffer[(40, 18)].bg, Color::Rgb(244, 240, 217));

        view.focus = Focus::Transcript;
        terminal
            .draw(|frame| render(frame, &view, &EffectiveKeymap::defaults()))
            .expect("Transcript focus should render");
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(40, 5)].bg, Color::Rgb(230, 226, 204));
        assert_eq!(buffer[(40, 18)].bg, Color::Rgb(244, 240, 217));

        view.focus = Focus::Search;
        terminal
            .draw(|frame| render(frame, &view, &EffectiveKeymap::defaults()))
            .expect("search focus should render");
        assert_eq!(
            terminal.backend().buffer()[(5, 23)].bg,
            Color::Rgb(230, 226, 204)
        );
    }

    #[test]
    fn wide_layout_does_not_render_empty_details() {
        let backend = TestBackend::new(140, 30);
        let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
        terminal
            .draw(|frame| render(frame, &view(Vec::new()), &EffectiveKeymap::defaults()))
            .expect("view should render");

        let rendered: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(!rendered.contains("Details"));
    }
}
