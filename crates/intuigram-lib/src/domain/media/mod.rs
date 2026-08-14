mod specialized;

pub use specialized::*;

/// Major media and specialized Message families.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaKind {
    /// Photo.
    Photo,
    /// Video document.
    Video,
    /// Animated document.
    Animation,
    /// Sticker or custom emoji document.
    Sticker,
    /// Generic file.
    File,
    /// Music or other audio document.
    Audio,
    /// Voice note.
    Voice,
    /// Video note.
    VideoNote,
    /// Web page preview.
    LinkPreview,
    /// Poll or quiz.
    Poll,
    /// Contact card.
    Contact,
    /// Static location.
    Location,
    /// Venue.
    Venue,
    /// Dice result.
    Dice,
    /// A location that Telegram is updating for a bounded period.
    LiveLocation,
    /// A Telegram game.
    Game,
    /// A Telegram invoice.
    Invoice,
    /// Telegram media guarded by a Stars price.
    PaidMedia,
    /// Telegram giveaway or published results.
    Giveaway,
    /// Telegram gift carried by a service Message.
    Gift,
    /// Story shared into a Message.
    Story,
    /// Collaborative Telegram TODO list.
    TodoList,
    /// Constructor not recognized by the current client.
    Unsupported,
}

/// One answer and its current aggregate result in a poll or quiz.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PollOptionView {
    /// Answer text.
    pub text: String,

    /// Number of votes, when Telegram exposes results.
    pub voters: Option<u32>,

    /// Whether the active Account chose this answer.
    pub chosen: bool,

    /// Whether Telegram marks this as a correct quiz answer.
    pub correct: bool,
}

/// Structured poll or quiz state retained by a Media Card.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PollView {
    /// Whether this is a quiz rather than an ordinary poll.
    pub quiz: bool,

    /// Whether more than one answer may be selected.
    pub multiple_choice: bool,

    /// Whether Telegram has closed voting.
    pub closed: bool,

    /// Aggregate voter count, when available.
    pub total_voters: Option<u32>,

    /// Answers in Telegram-defined order.
    pub options: Vec<PollOptionView>,

    /// Quiz explanation revealed by Telegram, when available.
    pub solution: Option<String>,
}

/// Transient option picker for voting in an open poll or quiz.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PollVoteView {
    /// Message containing the poll.
    pub message: super::MessageId,

    /// Currently targeted option.
    pub selected: usize,

    /// Options selected for submission.
    pub choices: Vec<usize>,

    /// Whether more than one option may be submitted.
    pub multiple_choice: bool,

    /// Display labels in Telegram-defined order.
    pub options: Vec<String>,
}

/// Text-first Media Card data used by every renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaCard {
    /// Semantic family.
    pub kind: MediaKind,

    /// Short type or filename label.
    pub title: String,

    /// Useful primary metadata or caption fallback.
    pub description: String,

    /// Additional text fallback lines such as a URL, author, or coordinates.
    pub details: Vec<String>,

    /// Poll or quiz state when this card represents one.
    pub poll: Option<PollView>,

    /// Structured state for Telegram's interactive Message families.
    pub specialized: Option<SpecializedMediaView>,

    /// Stable remote identifier used by download actions, when available.
    pub remote_id: Option<String>,
}

/// Durable Telegram-independent coordinates for one downloadable media file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaLocator {
    /// Data center that owns the file bytes.
    pub dc_id: i32,

    /// Stable cloud object identity and authorization material.
    pub source: MediaSource,

    /// User-facing filename chosen during normalization.
    pub name: String,

    /// Declared MIME type for the complete object.
    pub mime_type: String,

    /// Expected complete-object size.
    pub size: usize,

    /// Available bounded preview representations.
    pub thumbnails: Vec<MediaThumbnail>,
}

/// Stable cloud object identity needed to construct an upload file location.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MediaSource {
    /// Telegram photo identity.
    Photo {
        id: i64,
        access_hash: i64,
        file_reference: Vec<u8>,
    },

    /// Telegram document identity.
    Document {
        id: i64,
        access_hash: i64,
        file_reference: Vec<u8>,
    },
}

/// One selectable image representation attached to a media object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaThumbnail {
    /// Telegram size discriminator used in the file location.
    pub kind: String,

    /// Expected encoded byte size.
    pub size: usize,
}

impl MediaCard {
    /// Whether `body` is the generated text fallback for this Media Card.
    #[must_use]
    pub fn is_fallback_body(&self, body: &str) -> bool {
        let description = self.display_description();
        let fallback = if description.is_empty() {
            format!("[{}]", self.title)
        } else {
            format!("[{}] {description}", self.title)
        };
        body == fallback
    }

    /// Primary text generated from structured content when available.
    #[must_use]
    pub fn display_description(&self) -> String {
        match &self.specialized {
            Some(specialized) => specialized.display_description(),
            None => self.description.clone(),
        }
    }

    /// Secondary fallback lines generated from structured content when
    /// available.
    #[must_use]
    pub fn display_details(&self) -> Vec<String> {
        match &self.specialized {
            Some(specialized) => specialized.display_details(),
            None => self.details.clone(),
        }
    }
}

/// Small immutable RGBA image suitable for terminal-native preview rendering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InlineImage {
    width: u16,
    height: u16,
    rgba: Arc<[u8]>,
}

impl InlineImage {
    /// Builds an image when the dimensions exactly describe the RGBA payload.
    #[must_use]
    pub fn from_rgba(width: u16, height: u16, rgba: Vec<u8>) -> Option<Self> {
        let expected = usize::from(width)
            .checked_mul(usize::from(height))?
            .checked_mul(4)?;
        (rgba.len() == expected).then(|| Self {
            width,
            height,
            rgba: rgba.into(),
        })
    }

    /// Pixel width.
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.width
    }

    /// Pixel height.
    #[must_use]
    pub const fn height(&self) -> u16 {
        self.height
    }

    /// Row-major RGBA8 pixels.
    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}
use std::sync::Arc;
