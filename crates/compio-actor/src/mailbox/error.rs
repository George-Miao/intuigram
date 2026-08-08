use snafu::Snafu;

use super::Call;
use crate::Message;

/// A message rejected by a mailbox or broker.
#[derive(Debug, PartialEq, Eq, Snafu)]
#[snafu(module)]
pub enum DeliverError<M: Message> {
    /// The mailbox is at capacity.
    #[snafu(display("actor mailbox is full"))]
    Full {
        /// Rejected message.
        message: M,
    },

    /// The actor is stopping or has exited.
    #[snafu(display("actor mailbox is closed"))]
    Closed {
        /// Rejected message.
        message: M,
    },
}

impl<M: Message> DeliverError<M> {
    /// Recovers the rejected message.
    pub fn into_inner(self) -> M {
        match self {
            Self::Full { message } | Self::Closed { message } => message,
        }
    }
}

/// A call that could not be delivered or answered.
#[derive(Debug, PartialEq, Eq, Snafu)]
#[snafu(module)]
pub enum CallError<M: Message> {
    /// The mailbox was at capacity.
    #[snafu(display("actor mailbox is full"))]
    Full {
        /// Rejected request.
        message: M,
    },

    /// The actor was stopping or had exited.
    #[snafu(display("actor mailbox is closed"))]
    Closed {
        /// Rejected request.
        message: M,
    },

    /// The actor handled the request without replying.
    #[snafu(display("actor did not reply"))]
    NoReply,
}

impl<M: Message> CallError<M> {
    pub(super) fn from_deliver<R: Message>(error: DeliverError<Call<M, R>>) -> Self {
        match error {
            DeliverError::Full { message } => Self::Full {
                message: message.into_message(),
            },
            DeliverError::Closed { message } => Self::Closed {
                message: message.into_message(),
            },
        }
    }

    /// Recovers a request that was not delivered.
    pub fn into_inner(self) -> Option<M> {
        match self {
            Self::Full { message } | Self::Closed { message } => Some(message),
            Self::NoReply => None,
        }
    }
}
