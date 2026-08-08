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
    pub(super) const fn padded(self, area: ratatui::layout::Rect) -> ratatui::layout::Rect {
        match self {
            Self::Default if area.width > 2 && area.height > 2 => ratatui::layout::Rect::new(
                area.x.saturating_add(1),
                area.y.saturating_add(1),
                area.width.saturating_sub(2),
                area.height.saturating_sub(2),
            ),
            Self::Default | Self::Compact => area,
        }
    }

    pub(super) const fn horizontally_padded(
        self,
        area: ratatui::layout::Rect,
    ) -> ratatui::layout::Rect {
        match self {
            Self::Default if area.width > 2 => ratatui::layout::Rect::new(
                area.x.saturating_add(1),
                area.y,
                area.width.saturating_sub(2),
                area.height,
            ),
            Self::Default | Self::Compact => area,
        }
    }

    pub(super) const fn chat_list_area(self, area: ratatui::layout::Rect) -> ratatui::layout::Rect {
        match self {
            Self::Default if area.width > 1 && area.height > 2 => ratatui::layout::Rect::new(
                area.x,
                area.y.saturating_add(1),
                area.width.saturating_sub(1),
                area.height.saturating_sub(2),
            ),
            Self::Default | Self::Compact => area,
        }
    }

    pub(super) const fn item_height(self, content_height: u16) -> u16 {
        match self {
            Self::Default => content_height.saturating_add(1),
            Self::Compact => content_height,
        }
    }

    pub(super) const fn chat_header_height(self) -> u16 {
        match self {
            Self::Default => 3,
            Self::Compact => 2,
        }
    }

    pub(super) const fn active_chat_header_height(self) -> u16 {
        match self {
            Self::Default => 4,
            Self::Compact => 2,
        }
    }

    pub(super) const fn folder_height(self) -> u16 {
        match self {
            Self::Default => 3,
            Self::Compact => 1,
        }
    }

    pub(super) const fn chrome_row_height(self) -> u16 {
        match self {
            Self::Default => 3,
            Self::Compact => 1,
        }
    }
}
