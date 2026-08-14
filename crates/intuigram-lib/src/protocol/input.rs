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
    /// Confirmation that composition accepted one state-requested effect.
    EffectAccepted(EffectAdmission),

    /// Configures the normalized small-media admission supplied by composition.
    ConfigureSmallMediaCapacity(usize),
}

/// Effect families whose admission lets state offer more work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectAdmission {
    /// A visible image preview or avatar entered the small-media lane.
    SmallMedia,

    /// A notification entered independent local work.
    Notification,

    /// A read acknowledgement entered Telegram control work.
    ReadState,
}
