//! Ratatui terminal adapter and shared effective keymap.

use std::fmt::Write as _;
use std::io::{self, Stdout};

use crossterm::event::{self, Event, KeyCode as CrosstermKey, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use popgram_app::{Action, ConnectionState, DeliveryState, Focus, Intent, MessageDirection, View};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use snafu::{ResultExt, Snafu};

/// A terminal key independent of a concrete terminal backend.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Key {
    /// Printable character.
    Char(char),
    /// Up arrow.
    Up,

    /// Down arrow.
    Down,

    /// Home key.
    Home,

    /// End key.
    End,

    /// Enter key.
    Enter,

    /// Escape key.
    Escape,

    /// Tab key.
    Tab,
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
            Key::Home => "Home",
            Key::End => "End",
            Key::Enter => "Enter",
            Key::Escape => "Esc",
            Key::Tab => "Tab",
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
    binding(KeyChord::plain(Key::Tab), "Focus", Action::FocusNext, true),
    binding(KeyChord::plain(Key::Up), "Up", Action::MoveUp, true),
    binding(KeyChord::plain(Key::Down), "Down", Action::MoveDown, true),
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
        KeyChord::control(Key::Char('f')),
        "Search",
        Action::Search,
        true,
    ),
    binding(KeyChord::plain(Key::Escape), "Cancel", Action::Cancel, true),
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
}

/// Result returned by terminal operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Active alternate-screen terminal session.
pub struct TerminalUi {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    keymap: EffectiveKeymap,
}

impl TerminalUi {
    /// Enters raw mode and the alternate screen.
    pub fn enter() -> Result<Self> {
        enable_raw_mode().context(EnableRawModeSnafu)?;
        let mut stdout = io::stdout();
        if let Err(source) = execute!(stdout, EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(Error::EnterScreen { source });
        }
        let terminal =
            Terminal::new(CrosstermBackend::new(stdout)).context(InitializeTerminalSnafu)?;
        Ok(Self {
            terminal,
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

    /// Blocks until one application-relevant terminal event arrives.
    pub fn read_event(&self, view: &View) -> Result<UiEvent> {
        loop {
            match event::read().context(ReadEventSnafu)? {
                Event::Resize(..) => return Ok(UiEvent::Redraw),
                Event::Paste(text) => return Ok(UiEvent::Intent(Intent::Insert(text))),
                Event::Key(key)
                    if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
                {
                    let chord = chord_from_crossterm(key.code, key.modifiers);
                    if let Some(chord) = chord
                        && let Some(action) = self.keymap.resolve(view, chord)
                    {
                        return Ok(UiEvent::Intent(Intent::Action(action)));
                    }
                    match key.code {
                        CrosstermKey::Char(character)
                            if !key
                                .modifiers
                                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                        {
                            return Ok(UiEvent::Intent(Intent::Insert(character.to_string())));
                        }
                        CrosstermKey::Backspace => return Ok(UiEvent::Intent(Intent::Backspace)),
                        CrosstermKey::Enter => return Ok(UiEvent::Intent(Intent::Newline)),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
}

impl Drop for TerminalUi {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

fn chord_from_crossterm(code: CrosstermKey, modifiers: KeyModifiers) -> Option<KeyChord> {
    let key = match code {
        CrosstermKey::Char(character) => Key::Char(character.to_ascii_lowercase()),
        CrosstermKey::Up => Key::Up,
        CrosstermKey::Down => Key::Down,
        CrosstermKey::Home => Key::Home,
        CrosstermKey::End => Key::End,
        CrosstermKey::Enter => Key::Enter,
        CrosstermKey::Esc => Key::Escape,
        CrosstermKey::Tab | CrosstermKey::BackTab => Key::Tab,
        _ => return None,
    };
    Some(KeyChord {
        key,
        control: modifiers.contains(KeyModifiers::CONTROL),
        shift: modifiers.contains(KeyModifiers::SHIFT) || code == CrosstermKey::BackTab,
        alt: modifiers.contains(KeyModifiers::ALT),
    })
}

fn render(frame: &mut Frame<'_>, view: &View, keymap: &EffectiveKeymap) {
    let area = frame.area();
    let composer_height = if view.active_chat.is_some() { 3 } else { 0 };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(composer_height),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);
    render_main(frame, rows[0], view);
    if composer_height > 0 {
        render_composer(frame, rows[1], view);
    }
    render_folders(frame, rows[2], view);
    render_actions(frame, rows[3], view, keymap);
    render_status(frame, rows[4], view);
    if view.help_open {
        render_help(frame, area, view, keymap);
    }
}

fn render_main(frame: &mut Frame<'_>, area: Rect, view: &View) {
    if area.width < 72 {
        if matches!(view.focus, Focus::Chats | Focus::Folders) || view.active_chat.is_none() {
            render_chats(frame, area, view);
        } else {
            render_transcript(frame, area, view);
        }
        return;
    }
    let columns = if area.width >= 120 {
        Layout::horizontal([
            Constraint::Length(34),
            Constraint::Min(48),
            Constraint::Length(24),
        ])
        .split(area)
    } else {
        Layout::horizontal([Constraint::Length(32), Constraint::Min(40)]).split(area)
    };
    render_chats(frame, columns[0], view);
    render_transcript(frame, columns[1], view);
    if columns.len() == 3 {
        let detail = Paragraph::new("Details\n\nThreads and media will appear here.")
            .wrap(Wrap { trim: false })
            .block(Block::default().title(" Details ").borders(Borders::ALL));
        frame.render_widget(detail, columns[2]);
    }
}

fn render_chats(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let focused = view.focus == Focus::Chats;
    let items = view.chats.iter().enumerate().map(|(index, chat)| {
        let marker = if chat.pinned { "●" } else { " " };
        let unread = if chat.unread > 0 {
            format!(" {}", chat.unread)
        } else {
            String::new()
        };
        let style = selected_style(view.active_chat == Some(index), focused);
        ListItem::new(vec![
            Line::from(vec![
                Span::styled(
                    format!("{marker} {}", chat.title),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(unread, Style::default().fg(Color::Cyan)),
            ]),
            Line::from(Span::styled(
                chat.preview.clone(),
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .style(style)
    });
    let title = format!(" Chats · {} ", view.account_name);
    frame.render_widget(List::new(items).block(focused_block(title, focused)), area);
}

fn render_transcript(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let focused = view.focus == Focus::Transcript;
    let title = view
        .active_chat
        .and_then(|index| view.chats.get(index))
        .map_or_else(
            || " Transcript ".to_owned(),
            |chat| format!(" {} ", chat.title),
        );
    let items = view.messages.iter().enumerate().map(|(index, message)| {
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
            Span::styled(
                format!("{direction} {}", message.sender),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(reply, Style::default().fg(Color::Magenta)),
            Span::raw("  "),
            Span::styled(
                format!("{} {delivery}", message.timestamp),
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        ListItem::new(vec![header, Line::from(message.body.clone())])
            .style(selected_style(view.active_message == Some(index), focused))
    });
    frame.render_widget(List::new(items).block(focused_block(title, focused)), area);
}

fn render_composer(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let focused = view.focus == Focus::Composer;
    let title = view.composer.reply_to.map_or_else(
        || " Draft ".to_owned(),
        |message| format!(" Reply to {} ", message.0),
    );
    let text = if view.composer.text.is_empty() {
        Line::from(Span::styled(
            "Type or paste a message…",
            Style::default().fg(Color::DarkGray),
        ))
    } else {
        Line::from(view.composer.text.clone())
    };
    frame.render_widget(
        Paragraph::new(text)
            .block(focused_block(title, focused))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_folders(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let spans = view.folders.iter().enumerate().flat_map(|(index, folder)| {
        let style = if index == view.active_folder {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        let unread = if folder.unread > 0 {
            format!(" {}", folder.unread)
        } else {
            String::new()
        };
        [
            Span::styled(format!(" {}{unread} ", folder.title), style),
            Span::raw(" "),
        ]
    });
    frame.render_widget(Paragraph::new(Line::from(spans.collect::<Vec<_>>())), area);
}

fn render_actions(frame: &mut Frame<'_>, area: Rect, view: &View, keymap: &EffectiveKeymap) {
    let mut spans = Vec::new();
    for binding in keymap.action_bar(view) {
        spans.push(Span::styled(
            binding.key.label(),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(format!(" {}  ", binding.label)));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_status(frame: &mut Frame<'_>, area: Rect, view: &View) {
    let connection = match view.connection {
        ConnectionState::Connected => "connected",
        ConnectionState::Connecting => "connecting",
        ConnectionState::ReconnectCooldown => "reconnect cooldown",
    };
    let mut status = format!(" {} · {:?} · {connection}", view.account_name, view.focus);
    if let Some(search) = &view.search {
        write!(status, " · {:?} search: {}", search.scope, search.query)
            .expect("writing to a String cannot fail");
    }
    if view.has_newer_messages {
        status.push_str(" · new messages ↓");
    }
    frame.render_widget(
        Paragraph::new(status).style(Style::default().fg(Color::Black).bg(Color::DarkGray)),
        area,
    );
}

fn render_help(frame: &mut Frame<'_>, area: Rect, view: &View, keymap: &EffectiveKeymap) {
    let popup = centered_rect(70, 75, area);
    let lines = keymap.help(view).map(|binding| {
        Line::from(vec![
            Span::styled(
                format!("{:>12}", binding.key.label()),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw(format!("  {}", binding.label)),
        ])
    });
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines.collect::<Vec<_>>())
            .block(
                Block::default()
                    .title(" Context Help ")
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn focused_block(title: String, focused: bool) -> Block<'static> {
    let border = if focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    Block::default()
        .title(title)
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border))
}

fn selected_style(selected: bool, focused: bool) -> Style {
    if selected && focused {
        Style::default().fg(Color::Black).bg(Color::Cyan)
    } else if selected {
        Style::default().bg(Color::DarkGray)
    } else {
        Style::default()
    }
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
    use popgram_app::{Action, ComposerView, ConnectionState, Focus, SearchView, View};

    use super::{EffectiveKeymap, Key, KeyChord};

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
}
