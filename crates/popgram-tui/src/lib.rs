//! Ratatui terminal adapter and shared effective keymap.

use std::fmt::Write as _;
use std::io::{self, Stdout};
use std::time::Duration;

use compio_term::EventStream;
use crossterm::event::{self, Event, KeyCode as CrosstermKey, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use futures_util::StreamExt;
use popgram_app::{Action, ConnectionState, DeliveryState, Focus, Intent, MessageDirection, View};
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

const SURFACE_BACKGROUND: Color = Color::Rgb(16, 16, 16);
const FOCUSED_SURFACE_BACKGROUND: Color = Color::Rgb(28, 28, 28);
const CHROME_BACKGROUND: Color = Color::Rgb(8, 8, 8);

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
        KeyChord::control(Key::Char('q')),
        "Quit",
        Action::Quit,
        true,
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
    binding(KeyChord::plain(Key::Enter), "Open", Action::Open, true),
    binding(
        KeyChord::control(Key::Char('n')),
        "Draft",
        Action::Compose,
        true,
    ),
    binding(KeyChord::control(Key::Enter), "Send", Action::Send, true),
    binding(
        KeyChord::control(Key::Char('r')),
        "Reply",
        Action::Reply,
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

    /// Abort Popgram startup.
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
    events: EventStream,
}

impl TerminalUi {
    /// Enters raw mode and the alternate screen.
    pub fn enter() -> Result<Self> {
        let events = EventStream::new().context(InitializeEventStreamSnafu)?;
        Ok(Self {
            terminal: enter_terminal()?,
            keymap: EffectiveKeymap::defaults(),
            events,
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

    /// Waits asynchronously for one application-relevant terminal event.
    pub async fn next_event(&mut self, view: &View) -> Result<UiEvent> {
        loop {
            let event = self
                .events
                .next()
                .await
                .context(EventStreamClosedSnafu)?
                .context(StreamEventSnafu)?;
            if let Some(event) = resolve_event(&self.keymap, view, event) {
                return Ok(event);
            }
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
    match Terminal::new(CrosstermBackend::new(stdout)) {
        Ok(terminal) => Ok(terminal),
        Err(source) => {
            let mut stdout = io::stdout();
            let _ = execute!(stdout, LeaveAlternateScreen);
            let _ = disable_raw_mode();
            Err(Error::InitializeTerminal { source })
        }
    }
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) {
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();
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
                CrosstermKey::Enter => Some(UiEvent::Intent(Intent::Newline)),
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
                "Link Popgram to Telegram",
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
    }
}

fn render_main(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let columns = if area.width >= 120 {
        Layout::horizontal([
            Constraint::Length(32),
            Constraint::Length(1),
            Constraint::Min(48),
            Constraint::Length(1),
            Constraint::Length(24),
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
    if columns.len() == 5 {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    "Details",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Threads and media will appear here.",
                    Style::default().fg(Color::DarkGray),
                )),
            ])
            .style(surface_style(false))
            .wrap(Wrap { trim: false }),
            columns[4],
        );
    }
}

fn render_chats(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let focused = view.focus == Focus::Chats;
    let rows = Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).split(area);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Chats", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("  {}", view.account_name),
                Style::default().fg(Color::DarkGray),
            ),
        ]))
        .style(surface_style(focused)),
        rows[0],
    );
    let items = view.chats.iter().enumerate().map(|(index, chat)| {
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
                Span::styled(unread, Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                selection_rule(selected),
                Span::styled(chat.preview.clone(), Style::default().fg(Color::DarkGray)),
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
                    Style::default().fg(Color::DarkGray),
                ))
            },
            |chat| {
                Line::from(vec![
                    Span::styled(
                        chat.title.clone(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        if chat.unread > 0 {
                            format!("  {} unread", chat.unread)
                        } else {
                            "  up to date".to_owned()
                        },
                        Style::default().fg(Color::DarkGray),
                    ),
                ])
            },
        );
    frame.render_widget(
        Paragraph::new(header).style(surface_style(view.focus == Focus::Transcript)),
        rows[0],
    );
    render_transcript(frame, rows[1], view, view.focus == Focus::Transcript);
    if composer_height > 0 {
        render_composer(frame, rows[2], view);
    }
}

fn render_transcript(frame: &mut Frame<'_>, area: Rect, view: &View, focused: bool) {
    let items = view.messages.iter().enumerate().map(|(index, message)| {
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
        let header = Line::from(vec![
            selection_rule(selected),
            Span::styled(
                format!("{direction} {}", message.sender),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(reply, Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
            Span::styled(
                format!("{} {delivery}", message.timestamp),
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        ListItem::new(vec![
            header,
            Line::from(vec![
                selection_rule(selected),
                Span::raw(message.body.clone()),
            ]),
        ])
    });
    frame.render_widget(List::new(items).style(surface_style(focused)), area);
}

fn render_composer(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let focused = view.focus == Focus::Composer;
    let composer_label = view.composer.reply_to.map_or_else(
        || "Draft".to_owned(),
        |message| format!("Reply to {}", message.0),
    );
    let draft_content = if view.composer.text.is_empty() {
        Span::styled(
            "Type or paste a message…",
            Style::default().fg(Color::DarkGray),
        )
    } else {
        Span::raw(view.composer.text.clone())
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                interaction_rule(focused),
                Span::styled(composer_label, Style::default().fg(Color::DarkGray)),
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
            Style::default().fg(Color::Gray)
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
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
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
    let style = if view.focus == Focus::Search {
        surface_style(true)
    } else {
        Style::default().fg(Color::DarkGray).bg(CHROME_BACKGROUND)
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
                Style::default().fg(Color::Cyan),
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

fn selection_rule(selected: bool) -> Span<'static> {
    if selected {
        Span::styled("│ ", Style::default().fg(Color::Cyan))
    } else {
        Span::raw("  ")
    }
}

fn interaction_rule(active: bool) -> Span<'static> {
    if active {
        Span::styled("│ ", Style::default().fg(Color::Cyan))
    } else {
        Span::styled("│ ", Style::default().fg(Color::DarkGray))
    }
}

fn surface_style(focused: bool) -> Style {
    Style::default().fg(Color::Gray).bg(if focused {
        FOCUSED_SURFACE_BACKGROUND
    } else {
        SURFACE_BACKGROUND
    })
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
    use crossterm::event::{Event, KeyCode as CrosstermKey, KeyEvent, KeyEventKind, KeyModifiers};
    use popgram_app::{
        Action, ChatId, ChatView, ComposerView, ConnectionState, DeliveryState, Focus, FolderView,
        MessageDirection, MessageId, MessageView, SearchView, View,
    };
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;

    use super::{
        EffectiveKeymap, Key, KeyChord, UiEvent, chord_from_crossterm, qr_login_symbols, render,
        resolve_event,
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
            focus: Focus::Chats,
            composer: ComposerView::default(),
            search: None::<SearchView>,
            has_newer_messages: false,
            help_open: false,
            actions,
        }
    }

    #[test]
    fn displayed_action_bar_and_help_bindings_are_the_bindings_input_resolves() {
        let view = view(vec![Action::Search, Action::JumpLatest, Action::Help]);
        let keymap = EffectiveKeymap::defaults();

        for binding in keymap.help(&view) {
            assert_eq!(keymap.resolve(&view, binding.key), Some(binding.action));
            assert!(!binding.key.label().is_empty());
        }
        assert_eq!(
            keymap.resolve(&view, KeyChord::control(Key::Char('f'))),
            Some(Action::Search)
        );
        assert_eq!(
            keymap.resolve(&view, KeyChord::shift(Key::Down)),
            Some(Action::JumpLatest)
        );
        assert_eq!(
            keymap.resolve(&view, KeyChord::plain(Key::End)),
            Some(Action::JumpLatest)
        );
    }

    #[test]
    fn hierarchy_modifiers_resolve_only_in_their_effective_context() {
        let keymap = EffectiveKeymap::defaults();
        let chat_list = view(vec![
            Action::PreviousFolder,
            Action::NextFolder,
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
        let view = view(vec![Action::Search]);
        let keymap = EffectiveKeymap::defaults();

        assert_eq!(
            resolve_event(
                &keymap,
                &view,
                Event::Key(KeyEvent::new_with_kind(
                    CrosstermKey::Char('f'),
                    KeyModifiers::CONTROL,
                    KeyEventKind::Press,
                )),
            ),
            Some(UiEvent::Intent(popgram_app::Intent::Action(Action::Search)))
        );
        assert_eq!(
            resolve_event(&keymap, &view, Event::Paste("hello".to_owned())),
            Some(UiEvent::Intent(popgram_app::Intent::Insert(
                "hello".to_owned()
            )))
        );
        assert_eq!(
            resolve_event(&keymap, &view, Event::Resize(100, 30)),
            Some(UiEvent::Redraw)
        );
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
            title: "Popgram".to_owned(),
            preview: "daily driver".to_owned(),
            unread: 1,
            pinned: true,
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
        assert_eq!(buffer[(39, 2)].fg, Color::DarkGray);
        assert_eq!(buffer[(31, 18)].symbol(), "│");
        assert_eq!(buffer[(33, 18)].symbol(), "D");
        assert_eq!(buffer[(5, 5)].bg, Color::Rgb(16, 16, 16));
        assert_eq!(buffer[(40, 5)].bg, Color::Rgb(16, 16, 16));
        assert_eq!(buffer[(40, 18)].bg, Color::Rgb(28, 28, 28));
        assert_eq!(buffer[(30, 5)].bg, Color::Reset);
        assert_eq!(buffer[(5, 21)].bg, Color::Rgb(16, 16, 16));
        assert_eq!(buffer[(5, 22)].bg, Color::Rgb(16, 16, 16));
        assert_eq!(buffer[(5, 23)].bg, Color::Rgb(8, 8, 8));
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
        assert_eq!(buffer[(5, 5)].bg, Color::Rgb(28, 28, 28));
        assert_eq!(buffer[(40, 18)].bg, Color::Rgb(16, 16, 16));

        view.focus = Focus::Transcript;
        terminal
            .draw(|frame| render(frame, &view, &EffectiveKeymap::defaults()))
            .expect("Transcript focus should render");
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(40, 5)].bg, Color::Rgb(28, 28, 28));
        assert_eq!(buffer[(40, 18)].bg, Color::Rgb(16, 16, 16));

        view.focus = Focus::Search;
        terminal
            .draw(|frame| render(frame, &view, &EffectiveKeymap::defaults()))
            .expect("search focus should render");
        assert_eq!(
            terminal.backend().buffer()[(5, 23)].bg,
            Color::Rgb(28, 28, 28)
        );
    }
}
