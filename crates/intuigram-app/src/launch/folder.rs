use super::{Command, Result};
use crate::{Maintenance, parse_folder_maintenance};

impl Command {
    /// Creates a Folder from a title and comma-separated rules.
    pub fn folder_create(title: String, rules: String) -> Result<Self> {
        Self::folder("folder create", "create", [title, rules])
    }

    /// Renames one custom Folder.
    pub fn folder_rename(folder: String, title: String) -> Result<Self> {
        Self::folder("folder rename", "rename", [folder, title])
    }

    /// Moves one custom Folder to a zero-based position.
    pub fn folder_reorder(folder: String, position: String) -> Result<Self> {
        Self::folder("folder reorder", "reorder", [folder, position])
    }

    /// Exports a share link for one Folder.
    pub fn folder_share(folder: String) -> Result<Self> {
        Self::folder("folder share", "share", [folder])
    }

    /// Deletes one Folder without deleting its Chats.
    pub fn folder_delete(folder: String) -> Result<Self> {
        Self::folder("folder delete", "delete", [folder])
    }

    /// Replaces one Folder's inclusion and exclusion rules.
    pub fn folder_rules(folder: String, rules: String) -> Result<Self> {
        Self::folder("folder rules", "rules", [folder, rules])
    }

    fn folder<const N: usize>(label: &str, action: &str, values: [String; N]) -> Result<Self> {
        let command = parse_folder_maintenance(&mut values.into_iter(), action, label)?;
        Ok(Self::maintenance(Maintenance::Folder(command)))
    }
}
