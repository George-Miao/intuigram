use super::{ChatView, FolderView};

/// Telegram category and exclusion rules for an ordinary custom Folder.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FolderRulesView {
    pub contacts: bool,
    pub non_contacts: bool,
    pub groups: bool,
    pub broadcasts: bool,
    pub bots: bool,
    pub exclude_muted: bool,
    pub exclude_read: bool,
    pub exclude_archived: bool,
}

impl FolderRulesView {
    pub(crate) fn toggle(&mut self, index: usize) {
        let rule = match index {
            0 => &mut self.contacts,
            1 => &mut self.non_contacts,
            2 => &mut self.groups,
            3 => &mut self.broadcasts,
            4 => &mut self.bots,
            5 => &mut self.exclude_muted,
            6 => &mut self.exclude_read,
            7 => &mut self.exclude_archived,
            _ => return,
        };
        *rule = !*rule;
    }
}

/// Editable metadata for one custom Folder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FolderDetailsView {
    pub id: i32,
    pub rules: Option<FolderRulesView>,
    pub shareable: bool,
}

/// Folder create or edit form owned by the application state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FolderEditorView {
    pub id: Option<i32>,
    pub title: String,
    pub rules: Option<FolderRulesView>,
    /// Title row followed by eight rule rows.
    pub selected: usize,
}

/// Folder lifecycle overlay state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FolderManagerView {
    pub selected: usize,
    pub editor: Option<FolderEditorView>,
    pub delete_confirmation: Option<i32>,
    pub pending: bool,
}

/// Folder operation requested from Telegram.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FolderOperation {
    Create {
        title: String,
        rules: FolderRulesView,
    },
    Update {
        id: i32,
        title: String,
        rules: Option<FolderRulesView>,
    },
    Reorder {
        id: i32,
        position: usize,
    },
    Share {
        id: i32,
    },
    Delete {
        id: i32,
    },
}

/// Successful Telegram Folder mutation normalized for reconciliation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FolderOperationResult {
    Created {
        id: i32,
        title: String,
        rules: FolderRulesView,
    },
    Updated {
        id: i32,
        title: String,
        rules: Option<FolderRulesView>,
    },
    Reordered {
        id: i32,
        position: usize,
    },
    Shared {
        id: i32,
        url: String,
    },
    Deleted {
        id: i32,
    },
}

/// Fresh Telegram Folder projection fetched after a successful mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FolderReconciliation {
    pub folders: Vec<FolderView>,
    pub details: Vec<FolderDetailsView>,
    pub chats: Vec<ChatView>,
}
