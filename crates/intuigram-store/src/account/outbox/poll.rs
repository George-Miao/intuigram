use super::{OutboxId, OutboxRecord};

/// Result of polling the durable Outbox's single FIFO execution head.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutboxPoll {
    /// The head was claimed and is ready for adapter execution.
    Claimed(OutboxRecord),

    /// An already claimed or cancellation-pending item must finish first.
    Busy { id: OutboxId },

    /// The deferred head blocks newer work until this Unix timestamp.
    WaitingUntil { id: OutboxId, available_at: i64 },

    /// No nonterminal work is currently claimable.
    Idle,
}
