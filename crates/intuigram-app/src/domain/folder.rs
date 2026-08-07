use super::{ChatView, FolderView};

/// Stable Telegram identity for one custom Folder lifecycle operation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FolderId(pub i32);

/// Telegram category and exclusion rules for an ordinary custom Folder.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FolderRulesView {
    /// Include contacts.
    pub contacts: bool,

    /// Include users outside the Account's contacts.
    pub non_contacts: bool,

    /// Include Basic Groups and Supergroups.
    pub groups: bool,

    /// Include broadcast Channels.
    pub broadcasts: bool,

    /// Include bot Private Chats.
    pub bots: bool,

    /// Exclude muted Chats.
    pub exclude_muted: bool,

    /// Exclude Chats with no unread Messages.
    pub exclude_read: bool,

    /// Exclude archived Chats.
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
    /// Telegram custom Folder identity.
    pub id: FolderId,

    /// Editable rules, absent for shared Chat-list Folders.
    pub rules: Option<FolderRulesView>,

    /// Whether Telegram permits sharing this Folder.
    pub shareable: bool,
}

/// Folder create or edit form owned by the application state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FolderEditorView {
    /// Existing Folder identity, or `None` while creating.
    pub id: Option<FolderId>,

    /// User-facing Folder title.
    pub title: String,

    /// Editable category and exclusion rules.
    pub rules: Option<FolderRulesView>,

    /// Title row followed by eight rule rows.
    pub selected: usize,
}

/// Folder lifecycle overlay state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FolderManagerView {
    /// Active Folder row.
    pub selected: usize,

    /// Nested create or edit form.
    pub editor: Option<FolderEditorView>,

    /// Folder awaiting explicit deletion confirmation.
    pub delete_confirmation: Option<FolderId>,

    /// Whether a Telegram mutation is pending.
    pub pending: bool,
}

/// Folder operation requested from Telegram.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FolderOperation {
    /// Create a custom Folder from a title and category rules.
    Create {
        /// New Folder title.
        title: String,

        /// New Folder rules.
        rules: FolderRulesView,
    },
    /// Replace an existing Folder's title and rules.
    Update {
        /// Target Folder.
        id: FolderId,

        /// Replacement title.
        title: String,

        /// Replacement rules, absent for shared Folders.
        rules: Option<FolderRulesView>,
    },
    /// Move a Folder in Telegram's custom Folder order.
    Reorder {
        /// Target Folder.
        id: FolderId,

        /// Zero-based custom Folder position.
        position: usize,
    },
    /// Export a shareable Folder link.
    Share {
        /// Target Folder.
        id: FolderId,
    },
    /// Delete a Folder without deleting its Chats or Messages.
    Delete {
        /// Target Folder.
        id: FolderId,
    },
}

/// Successful Telegram Folder mutation normalized for reconciliation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FolderOperationResult {
    /// Telegram created a custom Folder and assigned its identity.
    Created {
        /// Telegram-assigned Folder identity.
        id: FolderId,

        /// Accepted title.
        title: String,

        /// Accepted rules.
        rules: FolderRulesView,
    },
    /// Existing Folder settings were accepted.
    Updated {
        /// Target Folder.
        id: FolderId,

        /// Accepted title.
        title: String,

        /// Accepted rules.
        rules: Option<FolderRulesView>,
    },
    /// Folder order was accepted.
    Reordered {
        /// Target Folder.
        id: FolderId,

        /// Accepted zero-based position.
        position: usize,
    },
    /// Telegram exported a Folder invitation.
    Shared {
        /// Target Folder.
        id: FolderId,

        /// Telegram share URL.
        url: String,
    },
    /// Telegram deleted the Folder definition.
    Deleted {
        /// Deleted Folder.
        id: FolderId,
    },
}

/// Fresh Telegram Folder projection fetched after a successful mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FolderReconciliation {
    /// Fresh Folder strip.
    pub folders: Vec<FolderView>,

    /// Fresh editable Folder metadata.
    pub details: Vec<FolderDetailsView>,

    /// Fresh Chat memberships used to update projections.
    pub chats: Vec<ChatView>,
}
