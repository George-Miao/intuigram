use super::*;

impl Effect {
    /// Returns the family that can offer follow-up work after composition
    /// accepts this effect.
    #[must_use]
    pub const fn admission(&self) -> Option<EffectAdmission> {
        match self {
            Self::LoadMediaPreview { .. } | Self::LoadAvatar { .. } => {
                Some(EffectAdmission::SmallMedia)
            }
            Self::Notify { .. } => Some(EffectAdmission::Notification),
            Self::ReadHistory { .. } | Self::ReadThread { .. } => Some(EffectAdmission::ReadState),
            _ => None,
        }
    }
}
