/// Event produced by the terminal adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UiEvent {
    /// Application intent resolved from a key or paste event.
    Intent(Intent),
    /// Terminal dimensions changed and the view should be redrawn.
    Redraw,
}

/// User-facing role emitted by the renderer for semantic behavior locators.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticRole {
    /// The scrollable Chat-list region.
    ChatList,

    /// One Chat-list entry.
    Chat,

    /// One forum Topic entry.
    Topic,

    /// The Active Chat's Topic list.
    TopicList,

    /// The scrollable Transcript region.
    Transcript,

    /// One visible Transcript Message.
    Message,

    /// One visible Message Media Card.
    MediaCard,

    /// The active Chat Composer.
    Composer,

    /// One Folder-strip entry.
    Folder,

    /// One currently available Action Bar entry.
    Action,
}

/// Semantic node produced during the same pass as terminal cells.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticNode {
    /// User-facing role.
    pub role: SemanticRole,

    /// User-facing name or text used by locators.
    pub name: String,

    /// Secondary user-facing description where applicable.
    pub description: Option<String>,

    /// Stable Chat or Message identifier where applicable.
    pub domain_id: Option<i64>,

    /// Action represented by an Action Bar node.
    pub action: Option<Action>,

    /// Message delivery state where applicable.
    pub delivery: Option<DeliveryState>,

    /// Whether this node is the current item.
    pub active: bool,

    /// Whether this Message belongs to the explicit Message Selection.
    pub selected: bool,

    /// Whether interaction currently targets this node or region.
    pub focused: bool,

    /// Cell bounds occupied by the node in this frame.
    pub bounds: Rect,
}

/// In-memory terminal frame and its matching semantic tree.
#[derive(Clone, Debug)]
pub struct TestFrame {
    /// Exact Ratatui cell buffer.
    pub buffer: Buffer,

    /// Semantic nodes generated while laying out the buffer.
    pub semantics: Vec<SemanticNode>,
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

    /// Mouse event reporting could not be enabled.
    #[snafu(display("failed to enable terminal mouse reporting"))]
    EnableMouseReporting {
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

    /// Terminal graphics could not be prepared or presented.
    #[snafu(display("failed to render terminal graphics"))]
    Graphics {
        /// Structured graphics adapter failure.
        source: GraphicsError,
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
use super::*;
