/// Density used by Chat, Message, Folder, and Composer presentation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ViewMode {
    /// Readable spacing with separation between items.
    #[default]
    Default,

    /// Original dense presentation.
    Compact,
}

impl ViewMode {
    pub(super) const fn item_height(self, content_height: u16) -> u16 {
        match self {
            Self::Default => content_height.saturating_add(1),
            Self::Compact => content_height,
        }
    }

    pub(super) const fn folder_height(self) -> u16 {
        match self {
            Self::Default => 3,
            Self::Compact => 1,
        }
    }
}
