/// Raster presentation selected for the current terminal environment.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Protocol {
    /// Portable colored half-block cells.
    #[default]
    Text,

    /// Kitty image transmission with Unicode virtual placements.
    KittyUnicode,

    /// Cursor-anchored Kitty image placement.
    KittyLegacy,

    /// OSC 1337 inline images.
    Iterm2,

    /// DEC Sixel graphics.
    Sixel,

    /// External Überzug++ overlay process.
    Ueberzug,

    /// External Chafa text renderer.
    Chafa,
}

impl Protocol {
    /// Whether the terminal consumes encoded image bytes directly.
    #[must_use]
    pub const fn is_native(self) -> bool {
        matches!(
            self,
            Self::KittyUnicode | Self::KittyLegacy | Self::Iterm2 | Self::Sixel
        )
    }

    /// Whether the renderer needs an explicit image placement for this
    /// protocol.
    #[must_use]
    pub const fn uses_placements(self) -> bool {
        !matches!(self, Self::Text)
    }

    /// Whether a virtual placement must exist before placeholder text is drawn.
    #[must_use]
    pub const fn uses_unicode_placeholders(self) -> bool {
        matches!(self, Self::KittyUnicode)
    }
}
