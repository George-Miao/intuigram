use std::num::NonZeroU32;

/// Stable nonzero terminal image identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ImageId(NonZeroU32);

impl ImageId {
    /// Creates an identifier when the raw value is nonzero.
    #[must_use]
    pub const fn new(value: u32) -> Option<Self> {
        match NonZeroU32::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Returns the protocol-level integer value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }
}
