/// Independent Telegram synchronization namespace.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum UpdateScope {
    /// Account-wide `pts`, `qts`, date, and sequence state.
    #[default]
    Account,

    /// Channel-local `pts` state.
    Channel(ChatId),
}

impl UpdateScope {
    /// Returns the stable persistence key for this synchronization namespace.
    #[must_use]
    pub fn storage_key(self) -> String {
        match self {
            Self::Account => "account".to_owned(),
            Self::Channel(chat) => format!("channel:{}", chat.0),
        }
    }
}

/// Telegram synchronization position accompanying normalized live events.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UpdateCursor {
    /// Independent Account or Channel cursor namespace.
    pub scope: UpdateScope,

    /// Latest persistent update timestamp when supplied by this envelope.
    pub pts: Option<i32>,

    /// Number of persistent events represented by this `pts` transition.
    pub pts_count: i32,

    /// Latest secret update timestamp when supplied by this envelope.
    pub qts: Option<i32>,

    /// Telegram server date when supplied by this envelope.
    pub date: Option<i32>,

    /// Latest global update sequence when supplied by this envelope.
    pub seq: Option<i32>,

    /// Sequence at which a combined update envelope begins.
    pub seq_start: Option<i32>,

    /// Telegram explicitly reported that this scope has missing updates.
    pub gap: bool,
}

/// One normalized adapter event with its durable cursor delta.
pub struct LiveEvent {
    /// Intuigram-owned events from one Telegram update envelope.
    pub events: Vec<AdapterEvent>,

    /// Scoped cursor fields advanced by the same Telegram envelope.
    pub cursors: Vec<UpdateCursor>,

    /// Opaque operation addresses learned from the same envelope.
    pub peers: PeerDirectory,
}

impl QrLoginToken {
    /// Returns the `tg://login` URI encoded by the QR symbol.
    #[must_use]
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Returns the token expiry as a Unix timestamp.
    #[must_use]
    pub const fn expires_at(&self) -> i32 {
        self.expires_at
    }
}

/// Opaque continuation required when Telegram moves QR login to another data
/// center.
pub struct QrLoginMigration {
    pub(super) dc_id: i32,
    pub(super) token: Vec<u8>,
}

impl QrLoginMigration {
    /// Returns the target Telegram data-center number.
    #[must_use]
    pub const fn dc_id(&self) -> i32 {
        self.dc_id
    }
}

/// Current state of Telegram QR authentication.
pub enum QrLogin {
    /// Display this token while waiting for another Telegram client to scan it.
    Pending(QrLoginToken),

    /// Continue authorization on another Telegram data center.
    Migrate(QrLoginMigration),

    /// The scanned Account requires its Telegram 2FA password.
    PasswordRequired(PasswordPrompt),

    /// Authentication completed.
    Authorized(AuthorizedUser),
}

/// Authorization and connection state that must survive process restarts.
#[derive(Clone, Eq, PartialEq)]
pub struct Session {
    /// Telegram data-center number.
    pub dc_id: i32,

    /// Direct endpoint selected for the data center.
    pub endpoint: SocketAddr,

    /// Secret authorization key.
    pub(super) auth_key: [u8; 256],

    /// Difference between local and Telegram server time.
    pub time_offset: i32,

    /// Initial server salt established by the key exchange.
    pub first_salt: i64,
}

impl Session {
    /// Reconstructs a session loaded from protected Account storage.
    #[must_use]
    pub const fn new(
        dc_id: i32,
        endpoint: SocketAddr,
        auth_key: [u8; 256],
        time_offset: i32,
        first_salt: i64,
    ) -> Self {
        Self {
            dc_id,
            endpoint,
            auth_key,
            time_offset,
            first_salt,
        }
    }

    /// Copies the key into durable Account storage.
    #[must_use]
    pub const fn auth_key(&self) -> [u8; 256] {
        self.auth_key
    }
}

impl fmt::Debug for Session {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Session")
            .field("dc_id", &self.dc_id)
            .field("endpoint", &self.endpoint)
            .field("auth_key", &"[REDACTED]")
            .field("time_offset", &self.time_offset)
            .field("first_salt", &self.first_salt)
            .finish()
    }
}

/// Result of sending a login code.
pub enum CodeRequest {
    /// Telegram delivered a code and expects it to be submitted.
    Sent(LoginCodeToken),
    /// This authorization key was already signed in.
    AlreadyAuthorized(AuthorizedUser),
}

/// Result of submitting a login code.
pub enum CodeSignIn {
    /// Authentication completed.
    Authorized(AuthorizedUser),
    /// Telegram requires the Account's 2FA password.
    PasswordRequired(PasswordPrompt),
}
use super::*;
