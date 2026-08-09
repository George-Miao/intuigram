use rasterm::Multiplexer;

use super::*;

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
    render_test_frame_for_protocol(
        view,
        width,
        height,
        GraphicsProtocol::Text,
        view_mode,
        &EffectiveKeymap::defaults(),
    )
    .0
}

#[cfg(test)]
pub(crate) fn render_test_frame_with_graphics(
    view: &View,
    width: u16,
    height: u16,
    protocol: GraphicsProtocol,
) -> (TestFrame, GraphicsFrame) {
    render_test_frame_for_protocol(
        view,
        width,
        height,
        protocol,
        ViewMode::Default,
        &EffectiveKeymap::defaults(),
    )
}

fn render_test_frame_for_protocol(
    view: &View,
    width: u16,
    height: u16,
    protocol: GraphicsProtocol,
    view_mode: ViewMode,
    keymap: &EffectiveKeymap,
) -> (TestFrame, GraphicsFrame) {
    render_test_frame_for_protocol_with_viewport(
        view,
        width,
        height,
        protocol,
        view_mode,
        keymap,
        &mut ChatViewport::default(),
    )
}

pub(super) fn render_test_frame_for_protocol_with_viewport(
    view: &View,
    width: u16,
    height: u16,
    protocol: GraphicsProtocol,
    view_mode: ViewMode,
    keymap: &EffectiveKeymap,
    chat_viewport: &mut ChatViewport,
) -> (TestFrame, GraphicsFrame) {
    let backend = TestBackend::new(width, height);
    let mut terminal =
        Terminal::new(backend).expect("Ratatui's in-memory TestBackend cannot fail initialization");
    let mut semantics = Vec::new();
    let mut graphics = GraphicsFrame::new(protocol, Multiplexer::None);
    terminal
        .draw(|frame| {
            render_with_graphics(
                frame,
                view,
                keymap,
                view_mode,
                &mut semantics,
                &mut graphics,
                chat_viewport,
            );
        })
        .expect("Ratatui's in-memory TestBackend cannot fail a draw");
    let buffer = terminal.backend().buffer().clone();
    graphics.locate(&buffer);
    (TestFrame { buffer, semantics }, graphics)
}

/// Stateful in-memory renderer for behavior tests spanning multiple frames.
#[derive(Debug, Default)]
pub struct TestRenderer {
    chat_viewport: ChatViewport,
}

impl TestRenderer {
    /// Renders one frame while preserving renderer-owned viewport state.
    #[must_use]
    pub fn render(&mut self, view: &View, width: u16, height: u16) -> TestFrame {
        render_test_frame_for_protocol_with_viewport(
            view,
            width,
            height,
            GraphicsProtocol::Text,
            ViewMode::Default,
            &EffectiveKeymap::defaults(),
            &mut self.chat_viewport,
        )
        .0
    }
}
