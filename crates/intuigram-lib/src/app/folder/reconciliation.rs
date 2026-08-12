use super::*;

impl App {
    pub(in crate::app) fn apply_folder_adapter_event(
        &mut self,
        event: AdapterEvent,
    ) -> Option<Effect> {
        match event {
            AdapterEvent::FolderMembershipChanged {
                chat,
                folder,
                included,
            } => {
                let effect = self.apply_folder_membership(chat, folder, included);
                self.view.notice = None;
                effect
            }
            AdapterEvent::FolderOperationCompleted {
                result,
                reconciliation,
            } => {
                self.view.notice = None;
                let needs_refresh = reconciliation.is_none()
                    && !matches!(result, FolderOperationResult::Shared { .. });
                if let Some(reconciliation) = reconciliation {
                    self.apply_folder_reconciliation(*reconciliation);
                }
                self.apply_folder_operation(result);
                needs_refresh.then_some(Effect::RefreshFolders)
            }
            AdapterEvent::FolderReconciled(reconciliation) => {
                self.apply_folder_reconciliation(*reconciliation);
                self.view.notice = None;
                None
            }
            AdapterEvent::FolderReconciliationFailed(reason) => {
                self.view.notice = Some(format!(
                    "Folder change was accepted, but refreshing its memberships failed: {reason}"
                ));
                None
            }
            AdapterEvent::FolderOperationFailed(reason) => {
                if let Some(manager) = &mut self.view.folder_manager {
                    manager.pending = false;
                }
                self.view.notice = Some(reason);
                None
            }
            _ => None,
        }
    }

    fn apply_folder_operation(&mut self, result: FolderOperationResult) {
        if let Some(manager) = &mut self.view.folder_manager {
            manager.pending = false;
        }
        match result {
            FolderOperationResult::Created { id, title, rules } => {
                if !self.view.folders.iter().any(|folder| folder.id == id.0) {
                    let insert = self
                        .view
                        .folders
                        .iter()
                        .position(|folder| folder.id == -1)
                        .unwrap_or(self.view.folders.len());
                    self.view.folders.insert(
                        insert,
                        FolderView {
                            id: id.0,
                            title,
                            unread: 0,
                        },
                    );
                }
                if !self
                    .view
                    .folder_details
                    .iter()
                    .any(|details| details.id == id)
                {
                    self.view.folder_details.push(FolderDetailsView {
                        id,
                        rules: Some(rules),
                        shareable: true,
                    });
                }
                if let Some(manager) = &mut self.view.folder_manager {
                    manager.editor = None;
                    manager.selected = self.view.folder_details.len().saturating_sub(1);
                }
            }
            FolderOperationResult::Updated { id, title, rules } => {
                if let Some(folder) = self
                    .view
                    .folders
                    .iter_mut()
                    .find(|folder| folder.id == id.0)
                {
                    folder.title = title;
                }
                if let Some(details) = self
                    .view
                    .folder_details
                    .iter_mut()
                    .find(|details| details.id == id)
                {
                    details.rules = rules;
                }
                if let Some(manager) = &mut self.view.folder_manager {
                    manager.editor = None;
                }
            }
            FolderOperationResult::Reordered { id, position } => {
                self.reorder_folder_projection(id, position);
            }
            FolderOperationResult::Shared { id: _, url } => {
                self.view.notice = Some(format!("Folder share link: {url}"));
            }
            FolderOperationResult::Deleted { id } => self.remove_folder_projection(id),
        }
    }

    fn apply_folder_reconciliation(&mut self, fresh: FolderReconciliation) {
        let active = self
            .view
            .folders
            .get(self.view.active_folder)
            .map(|folder| folder.id);
        let preferred_chat = self.active_chat_id();
        self.view.folders = fresh.folders;
        self.view.folder_details = fresh.details;
        for chat in fresh.chats {
            if let Some(existing) = self
                .all_chats
                .iter_mut()
                .find(|existing| existing.id == chat.id)
            {
                existing.folders = chat.folders;
            }
        }
        self.view.active_folder = active
            .and_then(|id| self.view.folders.iter().position(|folder| folder.id == id))
            .or_else(|| self.view.folders.iter().position(|folder| folder.id == 0))
            .unwrap_or(0);
        self.refresh_folder_chats(preferred_chat);
    }

    fn reorder_folder_projection(&mut self, id: FolderId, position: usize) {
        let active = self
            .view
            .folders
            .get(self.view.active_folder)
            .map(|folder| folder.id);
        let Some(current) = self
            .view
            .folder_details
            .iter()
            .position(|details| details.id == id)
        else {
            return;
        };
        let details = self.view.folder_details.remove(current);
        let position = position.min(self.view.folder_details.len());
        self.view.folder_details.insert(position, details);
        let mut custom = self
            .view
            .folder_details
            .iter()
            .filter_map(|details| {
                self.view
                    .folders
                    .iter()
                    .find(|folder| folder.id == details.id.0)
                    .cloned()
            })
            .collect::<Vec<_>>();
        let archive = self
            .view
            .folders
            .iter()
            .find(|folder| folder.id == -1)
            .cloned();
        self.view.folders.retain(|folder| folder.id == 0);
        self.view.folders.append(&mut custom);
        self.view.folders.extend(archive);
        self.view.active_folder = active
            .and_then(|active| {
                self.view
                    .folders
                    .iter()
                    .position(|folder| folder.id == active)
            })
            .unwrap_or(0);
        if let Some(manager) = &mut self.view.folder_manager {
            manager.selected = position;
        }
    }

    fn remove_folder_projection(&mut self, id: FolderId) {
        let active_id = self
            .view
            .folders
            .get(self.view.active_folder)
            .map(|folder| folder.id);
        self.view.folders.retain(|folder| folder.id != id.0);
        self.view.folder_details.retain(|details| details.id != id);
        for chat in &mut self.all_chats {
            chat.folders.retain(|folder| *folder != id.0);
        }
        if active_id == Some(id.0) {
            self.view.active_folder = self
                .view
                .folders
                .iter()
                .position(|folder| folder.id == 0)
                .unwrap_or(0);
        }
        self.refresh_folder_chats(self.active_chat_id());
        if let Some(manager) = &mut self.view.folder_manager {
            manager.delete_confirmation = None;
            manager.selected = manager
                .selected
                .min(self.view.folder_details.len().saturating_sub(1));
        }
    }
}
