/// Active alternate-screen terminal session.
pub struct TerminalUi {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    keymap: EffectiveKeymap,
    view_mode: ViewMode,
    semantics: Vec<SemanticNode>,
}

impl TerminalUi {
    /// Enters raw mode and the alternate screen.
    pub fn enter() -> Result<Self> {
        Self::enter_with_mode(ViewMode::Default)
    }

    /// Enters the terminal using the configured presentation density.
    pub fn enter_with_mode(view_mode: ViewMode) -> Result<Self> {
        Ok(Self {
            terminal: enter_terminal()?,
            keymap: EffectiveKeymap::defaults(),
            view_mode,
            semantics: Vec::new(),
        })
    }

    /// Draws one immutable application view.
    pub fn draw(&mut self, view: &View) -> Result<()> {
        let keymap = &self.keymap;
        let view_mode = self.view_mode;
        let mut semantics = Vec::new();
        self.terminal
            .draw(|frame| {
                render_with_semantics(frame, view, keymap, view_mode, &mut semantics);
            })
            .context(DrawSnafu)?;
        self.semantics = semantics;
        Ok(())
    }

    /// Draws a blocking startup recovery decision without entering a second
    /// terminal session.
    pub fn draw_recovery(&mut self, view: &RecoveryView) -> Result<()> {
        self.semantics.clear();
        self.terminal
            .draw(|frame| recovery::render(frame, view))
            .context(DrawSnafu)?;
        Ok(())
    }

    /// Resolves a raw terminal event against the latest application view.
    #[must_use]
    pub fn resolve_event(&self, view: &View, event: Event) -> Option<UiEvent> {
        resolve_event_with_semantics(&self.keymap, view, event, &self.semantics)
    }
}

/// Resolves a raw terminal event through the production effective keymap.
///
/// This is the input boundary used by hermetic behavior tests. It intentionally
/// accepts a real Crossterm event so tests cover the same context-sensitive
/// resolution as the interactive terminal.
#[must_use]
pub fn resolve_test_event(view: &View, event: Event) -> Option<UiEvent> {
    resolve_event(&EffectiveKeymap::defaults(), view, event)
}

/// Resolves a raw event against the semantic regions from a matching test
/// frame.
#[must_use]
pub fn resolve_test_frame_event(view: &View, frame: &TestFrame, event: Event) -> Option<UiEvent> {
    resolve_event_with_semantics(&EffectiveKeymap::defaults(), view, event, &frame.semantics)
}

/// Renders an immutable application view into Ratatui's in-memory backend.
///
/// The returned buffer is the exact cell grid produced by the production
/// renderer at the requested terminal size.
#[must_use]
pub fn render_test_frame(view: &View, width: u16, height: u16) -> TestFrame {
    render_test_frame_with_mode(view, width, height, ViewMode::Default)
}

/// Renders a test frame using an explicit presentation density.
#[must_use]
pub fn render_test_frame_with_mode(
    view: &View,
    width: u16,
    height: u16,
    view_mode: ViewMode,
) -> TestFrame {
    let backend = TestBackend::new(width, height);
    let mut terminal =
        Terminal::new(backend).expect("Ratatui's in-memory TestBackend cannot fail initialization");
    let mut semantics = Vec::new();
    terminal
        .draw(|frame| {
            render_with_semantics(
                frame,
                view,
                &EffectiveKeymap::defaults(),
                view_mode,
                &mut semantics,
            );
        })
        .expect("Ratatui's in-memory TestBackend cannot fail a draw");
    TestFrame {
        buffer: terminal.backend().buffer().clone(),
        semantics,
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

pub(super) fn enter_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
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
    if let Err(source) = execute!(stdout, EnableMouseCapture) {
        let _ = execute!(stdout, PopKeyboardEnhancementFlags, LeaveAlternateScreen);
        let _ = disable_raw_mode();
        return Err(Error::EnableMouseReporting { source });
    }
    match Terminal::new(CrosstermBackend::new(stdout)) {
        Ok(terminal) => Ok(terminal),
        Err(source) => {
            let mut stdout = io::stdout();
            let _ = execute!(
                stdout,
                DisableMouseCapture,
                PopKeyboardEnhancementFlags,
                LeaveAlternateScreen
            );
            let _ = disable_raw_mode();
            Err(Error::InitializeTerminal { source })
        }
    }
}

pub(super) fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) {
    let _ = disable_raw_mode();
    let _ = execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        PopKeyboardEnhancementFlags,
        LeaveAlternateScreen
    );
    let _ = terminal.show_cursor();
}

pub(crate) const fn terminal_keyboard_flags() -> KeyboardEnhancementFlags {
    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        .union(KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES)
}

pub(crate) fn resolve_event(
    keymap: &EffectiveKeymap,
    view: &View,
    event: Event,
) -> Option<UiEvent> {
    resolve_event_with_semantics(keymap, view, event, &[])
}

fn resolve_event_with_semantics(
    keymap: &EffectiveKeymap,
    view: &View,
    event: Event,
    semantics: &[SemanticNode],
) -> Option<UiEvent> {
    match event {
        Event::Resize(..) => Some(UiEvent::Redraw),
        Event::FocusGained | Event::FocusLost => Some(UiEvent::Redraw),
        Event::Paste(text) => Some(UiEvent::Intent(Intent::Insert(text))),
        Event::Mouse(mouse)
            if mouse.kind == MouseEventKind::Down(MouseButton::Left)
                && mouse.modifiers == KeyModifiers::NONE
                && !overlay_open_for_pointer(view) =>
        {
            semantics
                .iter()
                .rev()
                .find(|node| contains(node.bounds, mouse.column, mouse.row))
                .and_then(activation_target)
                .map(Intent::Activate)
                .map(UiEvent::Intent)
        }
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
                CrosstermKey::Left
                    if view.focus == Focus::Composer && key.modifiers == KeyModifiers::NONE =>
                {
                    Some(UiEvent::Intent(Intent::MoveComposerCursor(
                        ComposerMovement::Left,
                    )))
                }
                CrosstermKey::Right
                    if view.focus == Focus::Composer && key.modifiers == KeyModifiers::NONE =>
                {
                    Some(UiEvent::Intent(Intent::MoveComposerCursor(
                        ComposerMovement::Right,
                    )))
                }
                CrosstermKey::Up
                    if view.focus == Focus::Composer && key.modifiers == KeyModifiers::NONE =>
                {
                    Some(UiEvent::Intent(Intent::MoveComposerCursor(
                        ComposerMovement::Up,
                    )))
                }
                CrosstermKey::Down
                    if view.focus == Focus::Composer && key.modifiers == KeyModifiers::NONE =>
                {
                    Some(UiEvent::Intent(Intent::MoveComposerCursor(
                        ComposerMovement::Down,
                    )))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn contains(bounds: Rect, column: u16, row: u16) -> bool {
    column >= bounds.x && column < bounds.right() && row >= bounds.y && row < bounds.bottom()
}

fn activation_target(node: &SemanticNode) -> Option<ActivationTarget> {
    match node.role {
        SemanticRole::Folder => node
            .domain_id
            .and_then(|folder| i32::try_from(folder).ok())
            .map(ActivationTarget::Folder),
        SemanticRole::Chat => node
            .domain_id
            .map(|chat| ActivationTarget::Chat(ChatId(chat))),
        SemanticRole::Message => node
            .domain_id
            .map(|message| ActivationTarget::Message(MessageId(message))),
        SemanticRole::Composer => Some(ActivationTarget::Composer),
        SemanticRole::MediaCard | SemanticRole::Action => None,
    }
}

fn overlay_open_for_pointer(view: &View) -> bool {
    view.help_open
        || view.rich_media.is_some()
        || view.attachment_path.is_some()
        || view.save_as.is_some()
        || view.folder_picker.is_some()
        || view.folder_manager.is_some()
        || view.account_picker.is_some()
        || view.account_confirmation.is_some()
        || view.delete_confirmation.is_some()
        || view.forward_picker.is_some()
        || view.reaction_picker.is_some()
        || view.poll_vote.is_some()
        || view.link_confirmation.is_some()
}
use super::*;
