use snafu::Snafu;

/// Failure while observing operation time or allocating an identity.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(super)))]
pub enum Error {
    /// The local wall clock precedes the Unix epoch.
    #[snafu(display("system clock is before the Unix epoch"))]
    SystemClockBeforeEpoch {
        /// Invalid system-clock duration.
        source: std::time::SystemTimeError,
    },

    /// The local wall clock does not fit Intuigram's timestamp domain.
    #[snafu(display("system clock is outside the signed Unix timestamp domain"))]
    SystemClockOverflow {
        /// Failed integer conversion.
        source: std::num::TryFromIntError,
    },

    /// The operating system could not supply secure entropy.
    #[snafu(display("failed to generate a secure Telegram operation ID"))]
    GenerateOperationId {
        /// Operating-system entropy failure.
        source: getrandom::Error,
    },

    /// Secure entropy repeatedly produced already-issued identities.
    #[snafu(display("could not allocate a unique Telegram operation ID"))]
    OperationIdCollisions,
}

/// Result returned by operation providers.
pub type Result<T, E = Error> = std::result::Result<T, E>;
