/// Failure while authenticating or invoking Telegram.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    /// Every configured Telegram transport route failed.
    #[snafu(display("failed to connect to Telegram at {endpoint}"))]
    Connect {
        /// Telegram data-center endpoint.
        endpoint: SocketAddr,
        /// Underlying transport failure.
        source: compio_mtproto::ProxyError,
    },

    /// One route did not complete MTProto initialization before its deadline.
    #[snafu(display("Telegram route initialization timed out at {endpoint}"))]
    RouteInitializationTimeout { endpoint: SocketAddr },

    /// A fresh `MTProto` authorization key could not be generated.
    #[snafu(display("failed to generate Telegram authorization key"))]
    GenerateKey {
        /// Underlying key-exchange failure.
        source: compio_mtproto::KeyExchangeError,
    },

    /// Telegram rejected an API invocation.
    #[snafu(display("Telegram API invocation failed"))]
    Invoke {
        /// Underlying encrypted invocation failure.
        source: InvocationError,
    },

    /// A passive Telegram update could not be decoded.
    #[snafu(display("Telegram update payload was invalid"))]
    DecodeUpdate {
        /// Underlying TL decoding failure.
        source: grammers_tl_types::deserialize::Error,
    },

    /// A serialized Telegram cloud peer could not be decoded.
    #[snafu(display("Telegram cloud peer payload was invalid"))]
    DecodePeer {
        /// Underlying TL decoding failure.
        source: grammers_tl_types::deserialize::Error,
    },

    /// A serialized Telegram Message media constructor could not be decoded.
    #[snafu(display("Telegram Message media payload was invalid"))]
    DecodeMedia {
        /// Underlying TL decoding failure.
        source: grammers_tl_types::deserialize::Error,
    },

    /// Telegram returned a login-code result requiring a paid official flow.
    #[snafu(display("Telegram requires a paid or official-client login-code flow"))]
    LoginPaymentRequired,

    /// Telegram requires Account creation in an official client.
    #[snafu(display("Telegram Account sign-up must be completed in an official client"))]
    SignUpRequired,

    /// Telegram returned an empty user where an authorized identity was
    /// required.
    #[snafu(display("Telegram returned an empty authorized user"))]
    EmptyAuthorizedUser,

    /// Telegram requested 2FA but did not supply complete SRP parameters.
    #[snafu(display("Telegram returned incomplete 2FA password parameters"))]
    IncompletePasswordParameters,

    /// Telegram supplied unsupported or unsafe SRP parameters.
    #[snafu(display("Telegram returned unsupported or unsafe 2FA parameters"))]
    UnsupportedPasswordAlgorithm,

    /// No 2FA challenge is pending.
    #[snafu(display("no Telegram 2FA challenge is pending"))]
    MissingPasswordChallenge,

    /// Telegram did not return complete dialog data for a zero-hash request.
    #[snafu(display("Telegram returned dialogs without dialog contents"))]
    DialogsNotModified,

    /// Telegram returned a full dialog page without a usable next-page offset.
    #[snafu(display("Telegram dialog page has no usable pagination offset"))]
    DialogOffsetUnavailable,

    /// The requested Chat is not present in the current Telegram peer cache.
    #[snafu(display("Telegram peer for Chat {chat_id} is unavailable"))]
    PeerUnavailable {
        /// Intuigram Chat identifier.
        chat_id: i64,
    },

    /// A requested Telegram Folder is no longer available.
    #[snafu(display("Telegram Folder {folder_id} is unavailable"))]
    FolderUnavailable {
        /// Missing Telegram Folder identifier.
        folder_id: i32,
    },

    /// Telegram declined a dialog-filter edit without an RPC error.
    #[snafu(display("Telegram declined the Folder membership change"))]
    FolderUpdateRejected,

    /// A built-in or shared Folder cannot use ordinary inclusion rules.
    #[snafu(display("Telegram Folder {folder_id} does not support editable inclusion rules"))]
    FolderRulesUnavailable {
        /// Folder whose rules cannot be edited.
        folder_id: i32,
    },

    /// Telegram has no free identifier for another custom Folder.
    #[snafu(display("Telegram has no available custom Folder slot"))]
    FolderLimitReached,

    /// Telegram did not advertise a venue-search bot for this Account.
    #[snafu(display("Telegram place search is unavailable for this Account"))]
    VenueSearchUnavailable,

    /// Telegram's advertised venue-search username did not resolve to a usable
    /// bot.
    #[snafu(display("Telegram's place-search provider is unavailable"))]
    VenueSearchBotUnavailable,

    /// A Intuigram Message ID could not be represented by Telegram's API.
    #[snafu(display("Message ID {message_id} is outside Telegram's signed 32-bit domain"))]
    InvalidMessageId {
        /// Invalid Intuigram Message ID.
        message_id: i64,
    },

    /// A successful send response omitted its random-ID correlation.
    #[snafu(display("Telegram send response omitted the server Message identity"))]
    SentMessageIdentityUnavailable,

    /// A rich-text entity could not fit Telegram's signed 32-bit offsets.
    #[snafu(display("rich-text entity range {offset}..+{length} is outside Telegram's domain"))]
    InvalidEntityRange {
        /// UTF-16 entity offset.
        offset: usize,

        /// UTF-16 entity length.
        length: usize,
    },

    /// Telegram rejected an uploaded file part without an RPC error.
    #[snafu(display("Telegram rejected upload part {part}"))]
    UploadPartRejected {
        /// Zero-based rejected part.
        part: i32,
    },

    /// Telegram no longer returned the requested Message.
    #[snafu(display("Telegram Message {message_id} is unavailable for media download"))]
    DownloadMessageUnavailable { message_id: i64 },

    /// The requested Message has no downloadable photo or document.
    #[snafu(display("Telegram Message {message_id} has no downloadable media"))]
    DownloadMediaUnavailable { message_id: i64 },

    /// The requested Message is not an open Telegram poll or quiz.
    #[snafu(display("Telegram Message {message_id} has no open poll to vote in"))]
    PollUnavailable { message_id: i64 },

    /// The selected option no longer exists in the Telegram poll.
    #[snafu(display("poll option {option} is unavailable in Message {message_id}"))]
    PollOptionUnavailable { message_id: i64, option: usize },

    /// The selected specialized Message no longer has the requested family.
    #[snafu(display("Telegram Message {message_id} no longer contains {family}"))]
    SpecializedMediaUnavailable {
        message_id: i64,
        family: &'static str,
    },

    /// Telegram did not return the requested peer-scoped Story.
    #[snafu(display("Telegram Story {story_id} for peer {peer_id} is unavailable"))]
    StoryUnavailable { peer_id: i64, story_id: i32 },

    /// The selected TODO item no longer exists.
    #[snafu(display("TODO item {item} is unavailable in Message {message_id}"))]
    TodoItemUnavailable { message_id: i64, item: i32 },

    /// Telegram advertised an invalid or unrepresentable media size.
    #[snafu(display("Telegram media size {size} cannot be downloaded on this platform"))]
    InvalidDownloadSize { size: i64 },

    /// A peer avatar exceeded the bounded automatic-download budget.
    #[snafu(display("Telegram avatar exceeds the {limit}-byte inline limit"))]
    AvatarTooLarge { limit: usize },

    /// Telegram redirected the file to its encrypted CDN protocol.
    #[snafu(display("Telegram CDN download redirection is not implemented yet"))]
    DownloadCdnRedirect,

    /// Telegram ended a file transfer before its advertised size.
    #[snafu(display("Telegram download ended at {actual} of {expected} bytes"))]
    IncompleteDownload { expected: usize, actual: usize },

    /// Telegram advertised no direct address for a media data center.
    #[snafu(display("Telegram media data center {dc_id} has no direct endpoint"))]
    MediaDataCenterUnavailable { dc_id: i32 },

    /// Phone authorization must continue on another Telegram data center.
    #[snafu(display("Telegram phone authorization must migrate to data center {dc_id}"))]
    PhoneMigration {
        /// Target Telegram data-center number.
        dc_id: i32,
    },

    /// Intuigram connected to Telegram's isolated test environment.
    #[snafu(display("connected to a Telegram test data center instead of production"))]
    TestDataCenter,
}

impl Error {
    pub(crate) fn requires_peer_refresh(&self) -> bool {
        match self {
            Self::PeerUnavailable { .. } => true,
            Self::Invoke {
                source: InvocationError::Rpc { message, .. },
            } => matches!(
                message.as_str(),
                "CHANNEL_INVALID" | "CHANNEL_PRIVATE" | "PEER_ID_INVALID"
            ),
            _ => false,
        }
    }
}

/// Result returned by Telegram operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    /// Reports whether the operation may be retried after reconnecting.
    #[must_use]
    pub const fn is_connection_failure(&self) -> bool {
        matches!(self, Self::Connect { .. })
            || matches!(self, Self::Invoke { source } if source.is_connection_failure())
    }

    /// Returns the target data center for a phone-login migration.
    #[must_use]
    pub const fn phone_migration_dc(&self) -> Option<i32> {
        match self {
            Self::PhoneMigration { dc_id } => Some(*dc_id),
            _ => None,
        }
    }

    /// Reports whether this connection reached Telegram's isolated test
    /// environment.
    #[must_use]
    pub const fn is_test_data_center(&self) -> bool {
        matches!(self, Self::TestDataCenter)
    }
}
use super::*;
