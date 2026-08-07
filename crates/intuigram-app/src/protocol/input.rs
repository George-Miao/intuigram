use super::*;

/// Ordered inputs to the state owner.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "adapter batches enter the synchronous reducer once; boxing every input would add \
              allocation to all terminal actions"
)]
pub enum Input {
    /// An action from the active user interface.
    Intent(Intent),
    /// A result from an external adapter.
    Adapter(AdapterEvent),
}
