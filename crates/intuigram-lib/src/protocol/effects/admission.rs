use super::*;

impl Effect {
    /// Returns the bounded admission family replenished when composition
    /// accepts this effect.
    #[must_use]
    pub const fn admission(&self) -> Option<EffectAdmission> {
        match self {
            Self::LoadMediaPreview { .. } | Self::LoadAvatar { .. } => {
                Some(EffectAdmission::SmallMedia)
            }
            _ => None,
        }
    }
}
