use super::Action;

/// One context-sensitive action offered by a selectable popup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionMenuItemView {
    /// Action invoked when this item is chosen.
    pub action: Action,

    /// User-facing action label.
    pub label: String,
}

/// Selectable context actions owned by the application state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionMenuView {
    /// User-facing popup heading.
    pub title: String,

    /// Zero-based active item.
    pub selected: usize,

    /// Actions valid when the popup was opened.
    pub items: Vec<ActionMenuItemView>,
}
