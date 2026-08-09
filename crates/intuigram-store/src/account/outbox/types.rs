use sha2::{Digest, Sha256};

/// Stable positive identity for one durable Outbox item.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OutboxId(i64);

impl OutboxId {
    pub(super) fn from_stored(value: i64) -> Option<Self> {
        (value > 0).then_some(Self(value))
    }

    /// Returns the SQLite-independent numeric identity.
    #[must_use]
    pub const fn get(self) -> i64 {
        self.0
    }
}

/// Replay category for a durable outbound operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutboxOperation {
    /// Idempotent remote object creation.
    Create,

    /// Idempotent Message send identified by its random ID.
    Send,

    /// Replay-unsafe mutation of an existing remote object.
    Mutation,
}

/// Durable lifecycle of one Outbox item.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutboxState {
    /// Eligible for FIFO execution.
    Ready,

    /// Claimed by the Telegram adapter.
    InFlight,

    /// Cancellation was requested while the adapter operation was in flight.
    CancelRequested,

    /// Waiting until its explicit retry time.
    Deferred,

    /// Permanently failed.
    Failed,

    /// Requires user resolution because safe replay cannot be proven.
    Conflict,

    /// The remote outcome cannot be determined and requires user resolution.
    OutcomeUnknown,

    /// Explicitly time-bounded work passed its deadline.
    Expired,

    /// Cancelled locally before acknowledgement.
    Cancelled,
}

/// Explicit lifetime of an admitted Outbox item.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutboxExpiry {
    /// Retain until acknowledged, cancelled, or explicitly failed.
    Never,

    /// Expire at this Unix timestamp.
    At(i64),
}

/// Versioned Intuigram-owned payload stored independently of adapter effects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutboxPayload {
    /// Initial stable payload representation.
    V1(OutboxPayloadV1),
}

impl OutboxPayload {
    pub(super) const fn scope(&self) -> (i64, Option<i64>, Option<i64>) {
        match self {
            Self::V1(payload) => (payload.chat_id, payload.thread_root, payload.saved_peer),
        }
    }
}

/// Initial Outbox payload representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutboxPayloadV1 {
    /// Destination Chat.
    pub chat_id: i64,

    /// Topic or thread root, when scoped below the root Chat.
    pub thread_root: Option<i64>,

    /// Saved Messages or monoforum peer scope.
    pub saved_peer: Option<i64>,

    /// Optimistic local Message identity, when the operation creates one.
    pub local_message_id: Option<i64>,

    /// Stable Telegram deduplication identity for create/send operations.
    pub random_id: i64,

    /// Version-owned semantic operation content.
    pub content: Vec<u8>,
}

/// Exact media retained with a durable Outbox item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutboxMedia {
    /// Original display file name.
    pub file_name: String,

    /// Intuigram-normalized MIME type.
    pub mime_type: String,

    /// Exact admitted bytes.
    pub bytes: Vec<u8>,

    /// SHA-256 of `bytes`, checked whenever the record is loaded.
    pub sha256: [u8; 32],
}

impl OutboxMedia {
    /// Captures media bytes together with their exact content hash.
    #[must_use]
    pub fn new(file_name: String, mime_type: String, bytes: Vec<u8>) -> Self {
        let sha256 = Sha256::digest(&bytes).into();
        Self {
            file_name,
            mime_type,
            bytes,
            sha256,
        }
    }

    pub(super) fn hash_is_valid(&self) -> bool {
        <[u8; 32]>::from(Sha256::digest(&self.bytes)) == self.sha256
    }
}

/// All local records committed while admitting one operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutboxAdmission {
    /// Replay category.
    pub operation: OutboxOperation,

    /// Versioned operation payload.
    pub payload: OutboxPayload,

    /// Exact media that must survive until acknowledgement.
    pub media: Vec<OutboxMedia>,

    /// Optimistic Message committed in the same transaction, when any.
    pub optimistic_message: Option<crate::account::StoredMessage>,

    /// Whether the payload's exact Draft scope is consumed atomically.
    pub consume_draft: bool,

    /// Unix timestamp establishing FIFO order.
    pub admitted_at: i64,

    /// Caller-chosen lifetime; `Never` imposes no implicit deadline.
    pub expiry: OutboxExpiry,
}

/// Complete durable Outbox item returned to runtime adapters and recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutboxRecord {
    /// Durable Outbox identity.
    pub id: OutboxId,

    /// Replay category.
    pub operation: OutboxOperation,

    /// Current lifecycle state.
    pub state: OutboxState,

    /// Versioned operation payload.
    pub payload: OutboxPayload,

    /// Exact retained media.
    pub media: Vec<OutboxMedia>,

    /// FIFO admission timestamp.
    pub admitted_at: i64,

    /// Earliest retry timestamp, when deferred.
    pub available_at: Option<i64>,

    /// Explicit expiry timestamp, when any.
    pub expires_at: Option<i64>,

    /// Number of claims made so far.
    pub attempts: u32,

    /// Last normalized adapter failure description, when any.
    pub last_error: Option<String>,
}
