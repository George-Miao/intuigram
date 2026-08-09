mod clock;
mod error;
mod ids;

pub use clock::SystemClock;
pub use error::{Error, Result};
pub use ids::SecureOperationIds;

/// Wall-clock source owned by the application composition layer.
pub trait Clock {
    /// Returns the current Unix timestamp in whole seconds.
    fn unix_seconds(&self) -> Result<i64>;
}

/// Source of Telegram operation deduplication identities.
pub trait OperationIdSource {
    /// Returns an identity not previously emitted by this source.
    fn next_id(&mut self) -> Result<i64>;
}

/// Time and identity observed for one Outbox admission or execution attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperationStamp {
    observed_at: i64,
    random_id: i64,
}

impl OperationStamp {
    /// Unix timestamp observed for this admission or replay attempt.
    #[must_use]
    pub const fn observed_at(self) -> i64 {
        self.observed_at
    }

    /// Stable Telegram deduplication identity.
    #[must_use]
    pub const fn random_id(self) -> i64 {
        self.random_id
    }
}

/// Composition-owned providers used while admitting and replaying operations.
pub struct OperationProviders {
    clock: Box<dyn Clock>,
    ids: Box<dyn OperationIdSource>,
}

impl OperationProviders {
    /// Creates providers from explicitly owned clock and identity sources.
    pub fn new(clock: impl Clock + 'static, ids: impl OperationIdSource + 'static) -> Self {
        Self {
            clock: Box::new(clock),
            ids: Box::new(ids),
        }
    }

    /// Creates providers backed by the system clock and operating-system
    /// entropy.
    #[must_use]
    pub fn production() -> Self {
        Self::new(SystemClock, SecureOperationIds::default())
    }

    /// Returns the current injected Unix time for defer and expiry decisions.
    pub fn now(&self) -> Result<i64> {
        self.clock.unix_seconds()
    }

    /// Admits a new operation with one fresh, stable random ID.
    pub fn admit(&mut self) -> Result<OperationStamp> {
        Ok(OperationStamp {
            observed_at: self.now()?,
            random_id: self.ids.next_id()?,
        })
    }

    /// Creates a replay attempt without consuming a new random ID.
    pub fn replay(&self, persisted_random_id: i64) -> Result<OperationStamp> {
        Ok(OperationStamp {
            observed_at: self.now()?,
            random_id: persisted_random_id,
        })
    }
}

impl Default for OperationProviders {
    fn default() -> Self {
        Self::production()
    }
}
