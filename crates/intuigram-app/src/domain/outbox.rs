use super::{ChatId, MessageId};

/// Store-independent identity of one durable outbound operation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OutboxKey(pub i64);

/// Durable lifecycle projected by an adapter into application state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutboxStateView {
    /// Eligible for ordered execution.
    Ready,

    /// Claimed by the Telegram adapter.
    InFlight,

    /// Waiting until a retry deadline.
    Deferred,

    /// Cancellation was requested while remote work was active.
    CancelRequested,

    /// A terminal adapter failure needs a safe retry or dismissal.
    Failed,

    /// The operation basis changed and needs explicit reconciliation.
    Conflict,

    /// The remote outcome is ambiguous and needs explicit resolution.
    OutcomeUnknown,

    /// A caller-chosen deadline passed before execution.
    Expired,

    /// Work was cancelled before acknowledgement.
    Cancelled,
}

/// User decision applied to one projected Outbox item.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutboxAction {
    /// Cancel unstarted work or request cancellation of active work.
    Cancel,

    /// Retry a failed operation whose replay is known to be safe.
    Retry,

    /// Reconcile a mutation with a fresh semantic basis before retrying.
    ResolveConflict,

    /// Explicitly retry after accepting an ambiguous remote outcome.
    ResolveOutcomeUnknown,

    /// Remove a terminal Outbox record while retaining local Messages.
    Dismiss,
}

/// Read-only durable operation data consumed by application state and UIs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutboxItemView {
    /// Stable Account-local identity.
    pub key: OutboxKey,

    /// Owning Chat.
    pub chat: ChatId,

    /// Optimistic local Message, when this operation creates one.
    pub local_message: Option<MessageId>,

    /// Current durable lifecycle.
    pub state: OutboxStateView,

    /// Whether ordinary retry is proven safe for this operation.
    pub retryable: bool,

    /// Earliest retry time, when deferred.
    pub available_at: Option<i64>,

    /// Caller-chosen expiry deadline, when any.
    pub expires_at: Option<i64>,

    /// Last normalized failure description, when any.
    pub last_error: Option<String>,
}
