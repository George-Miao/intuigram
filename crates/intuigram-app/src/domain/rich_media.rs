/// Telegram-owned library selected from the Composer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RichMediaLibraryKind {
    /// Recently used Telegram stickers.
    Stickers,
    /// Saved Telegram animations.
    Gifs,
    /// Account custom emoji documents.
    CustomEmoji,
}

/// Upload presentation chosen for a local file or recording.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RichMediaUploadKind {
    /// Compressed image.
    Photo,
    /// Streamable ordinary video.
    Video,
    /// Generic document preserving its bytes.
    File,
    /// Looping animation.
    Animation,
    /// Telegram sticker document.
    Sticker,
    /// Telegram custom emoji document.
    CustomEmoji,
    /// Push-to-talk audio note.
    Voice,
    /// Circular video note.
    VideoNote,
}

impl RichMediaUploadKind {
    pub(crate) const ALL: [Self; 8] = [
        Self::Photo,
        Self::Video,
        Self::File,
        Self::Animation,
        Self::Sticker,
        Self::CustomEmoji,
        Self::Voice,
        Self::VideoNote,
    ];

    pub(crate) fn next(self) -> Self {
        let index = Self::ALL.iter().position(|kind| *kind == self).unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
}

/// Opaque adapter-owned media-library entry.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RichMediaItemId(pub u64);

/// One library result safe for application and renderer use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RichMediaItemView {
    /// Adapter-owned identifier valid for the current session.
    pub id: RichMediaItemId,

    /// Safe human-readable media description.
    pub label: String,
}

/// Active rich-media Composer surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RichMediaComposerView {
    /// Current nested rich-media operation.
    pub mode: RichMediaComposerMode,

    /// Active row within that operation.
    pub selected: usize,

    /// Whether the adapter is loading selectable media.
    pub pending: bool,
}

/// Context nested under the rich-media Composer surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RichMediaComposerMode {
    /// Top-level operation chooser.
    Menu,
    /// Telegram-owned media library results.
    Library {
        /// Library being browsed.
        kind: RichMediaLibraryKind,

        /// Current normalized results.
        items: Vec<RichMediaItemView>,
    },
    /// Exact local file and its explicit upload semantics.
    File {
        /// Exact platform path without shell expansion.
        path: String,

        /// Telegram media presentation.
        kind: RichMediaUploadKind,
    },
    /// Voice or circular-video capture request.
    Recording {
        /// Supported recording output kind.
        kind: RichMediaUploadKind,

        /// User-entered duration in seconds.
        seconds: String,

        /// Platform capture device understood by ffmpeg.
        device: String,
    },
    /// Contact card fields.
    Contact {
        /// Telegram-compatible telephone number.
        phone: String,

        /// Required contact first name.
        first_name: String,

        /// Optional contact last name.
        last_name: String,
    },
}
