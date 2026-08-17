use rasterm::{CellPixels, Multiplexer};

/// Active alternate-screen terminal session.
pub struct TerminalUi {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    keymap: EffectiveKeymap,
    view_options: ViewOptions,
    semantics: Vec<SemanticNode>,
    frame_state: TerminalFrameState,
}

pub(crate) struct TerminalFrameState {
    protocol: GraphicsProtocol,
    multiplexer: Multiplexer,
    graphics: GraphicsWorker,
    chat_viewport: ChatViewport,
    cell_pixels: CellPixels,
}

impl TerminalFrameState {
    pub(crate) fn new(
        protocol: GraphicsProtocol,
        multiplexer: Multiplexer,
    ) -> graphics::Result<Self> {
        Ok(Self {
            protocol,
            multiplexer,
            graphics: GraphicsWorker::new(protocol)?,
            chat_viewport: ChatViewport::default(),
            cell_pixels: CellPixels::default(),
        })
    }

    fn with_cell_pixels(mut self, cell_pixels: CellPixels) -> Self {
        self.cell_pixels = cell_pixels;
        self
    }

    pub(crate) fn poll_redraw(&mut self, cx: &mut Context<'_>) -> Poll<graphics::Result<()>> {
        self.graphics.poll_ready(cx)
    }
}

impl TerminalUi {
    /// Enters raw mode and the alternate screen.
    pub fn enter() -> Result<Self> {
        Self::enter_with_options(ViewOptions::default())
    }

    /// Enters the terminal using the configured presentation density.
    pub fn enter_with_mode(view_mode: ViewMode) -> Result<Self> {
        Self::enter_with_options(ViewOptions {
            mode: view_mode,
            ..ViewOptions::default()
        })
    }

    /// Enters the terminal using resolved presentation settings.
    pub fn enter_with_options(view_options: ViewOptions) -> Result<Self> {
        let (graphics_protocol, graphics_multiplexer) = graphics::graphics_environment();
        Ok(Self {
            terminal: enter_terminal()?,
            keymap: EffectiveKeymap::defaults(),
            view_options,
            semantics: Vec::new(),
            frame_state: TerminalFrameState::new(graphics_protocol, graphics_multiplexer)
                .context(GraphicsSnafu)?
                .with_cell_pixels(terminal_cell_pixels()),
        })
    }

    /// Draws one immutable application view.
    pub fn draw(&mut self, view: &View) -> Result<()> {
        self.semantics = draw_terminal_view(
            &mut self.terminal,
            &mut self.frame_state,
            &self.keymap,
            self.view_options,
            view,
        )?;
        Ok(())
    }

    /// Returns avatar peers that occupy cells in the latest frame.
    #[must_use]
    pub fn visible_avatar_peers(&self, view: &View) -> Vec<ChatId> {
        visible_avatar_peers(view, &self.semantics)
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

    /// Polls completion of background terminal-graphics preparation.
    pub fn poll_redraw(&mut self, cx: &mut Context<'_>) -> Poll<Result<()>> {
        self.frame_state
            .poll_redraw(cx)
            .map(|result| result.context(GraphicsSnafu))
    }
}

fn terminal_cell_pixels() -> CellPixels {
    window_size()
        .ok()
        .and_then(|size| {
            CellPixels::from_terminal(size.width, size.height, size.columns, size.rows)
        })
        .unwrap_or_default()
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

pub(crate) fn draw_terminal_view<W: io::Write>(
    terminal: &mut Terminal<CrosstermBackend<W>>,
    state: &mut TerminalFrameState,
    keymap: &EffectiveKeymap,
    view_options: ViewOptions,
    view: &View,
) -> Result<Vec<SemanticNode>> {
    let prepared = if state.protocol.uses_placements() {
        let area = terminal.get_frame().area();
        let (frame, mut graphics) = test_renderer::render_test_frame_for_protocol_with_viewport(
            view,
            area.width,
            area.height,
            state.protocol,
            view_options,
            keymap,
            &mut state.chat_viewport,
        );
        graphics.set_multiplexer(state.multiplexer);
        graphics.set_cell_pixels(state.cell_pixels);
        Some((frame, graphics))
    } else {
        None
    };

    if let Some((_, graphics)) = &prepared {
        state
            .graphics
            .request(graphics.requests())
            .context(GraphicsSnafu)?;
    }
    let output = state.graphics.take_output().context(GraphicsSnafu)?;
    if state.protocol.uses_unicode_placeholders()
        && let Some(output) = &output
    {
        GraphicsWorker::write(terminal.backend_mut(), output).context(GraphicsSnafu)?;
        if let Some((frame, graphics)) = &prepared {
            redraw_graphics_cells(terminal.backend_mut(), &frame.buffer, graphics.requests())
                .context(DrawSnafu)?;
        }
    }

    let mut semantics = Vec::new();
    let mut graphics = GraphicsFrame::new(state.protocol, state.multiplexer);
    terminal
        .draw(|frame| {
            render_with_graphics(
                frame,
                view,
                keymap,
                view_options,
                &mut semantics,
                &mut graphics,
                &mut state.chat_viewport,
            );
        })
        .context(DrawSnafu)?;
    if !state.protocol.uses_unicode_placeholders()
        && let Some(output) = &output
    {
        GraphicsWorker::write(terminal.backend_mut(), output).context(GraphicsSnafu)?;
    }
    Ok(semantics)
}

fn redraw_graphics_cells<W: io::Write>(
    backend: &mut CrosstermBackend<W>,
    buffer: &Buffer,
    requests: &[GraphicsRequest],
) -> io::Result<()> {
    let area = buffer.area;
    let cells = requests.iter().flat_map(|request| {
        let left = request.x.max(area.left());
        let top = request.y.max(area.top());
        let right = request
            .x
            .saturating_add(request.size.columns)
            .min(area.right());
        let bottom = request
            .y
            .saturating_add(request.size.rows)
            .min(area.bottom());
        (top..bottom).flat_map(move |y| (left..right).map(move |x| (x, y, &buffer[(x, y)])))
    });
    backend.draw(cells)
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
        .union(KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS)
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
        Event::Mouse(mouse) => resolve_pointer(view, mouse, semantics),
        Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
            let chord = chord_from_crossterm(key.code, key.modifiers);
            if let Some(chord) = chord
                && let Some(action) = keymap.resolve(view, chord)
            {
                if action == Action::EditPrevious
                    && !semantics.is_empty()
                    && !previous_edit_is_visible(view, semantics)
                {
                    return Some(UiEvent::Intent(Intent::MoveComposerCursor(
                        ComposerMovement::Up,
                    )));
                }
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

fn previous_edit_is_visible(view: &View, semantics: &[SemanticNode]) -> bool {
    let Some(message) = view
        .messages
        .iter()
        .rev()
        .find(|message| message.direction == MessageDirection::Outgoing && message.id.0 > 0)
    else {
        return false;
    };

    semantics.iter().any(|node| {
        node.role == SemanticRole::Message
            && node.domain_id == Some(message.id.0)
            && node.bounds.height > 0
    })
}

use super::*;
