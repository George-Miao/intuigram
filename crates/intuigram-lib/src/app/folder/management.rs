use super::*;

impl App {
    pub(in crate::app) fn open_folder_manager(&mut self) {
        if self.view.connection == ConnectionState::Connected {
            self.view.folder_manager = Some(FolderManagerView {
                selected: 0,
                editor: None,
                delete_confirmation: None,
                pending: false,
            });
        }
    }

    pub(in crate::app) fn apply_folder_manager_action(&mut self, action: Action) -> Option<Effect> {
        if self
            .view
            .folder_manager
            .as_ref()
            .is_some_and(|manager| manager.pending)
        {
            return None;
        }
        if self
            .view
            .folder_manager
            .as_ref()
            .is_some_and(|manager| manager.delete_confirmation.is_some())
        {
            return self.apply_folder_delete_confirmation(action);
        }
        if self
            .view
            .folder_manager
            .as_ref()
            .is_some_and(|manager| manager.editor.is_some())
        {
            return self.apply_folder_editor(action);
        }
        self.apply_folder_list_action(action)
    }

    fn apply_folder_list_action(&mut self, action: Action) -> Option<Effect> {
        let count = self.view.folder_details.len();
        let selected = self
            .view
            .folder_manager
            .as_ref()
            .map_or(0, |manager| manager.selected);
        match action {
            Action::MoveUp | Action::MoveDown => {
                if let Some(manager) = &mut self.view.folder_manager {
                    manager.selected =
                        move_index(Some(selected), count, action == Action::MoveDown).unwrap_or(0);
                }
                None
            }
            Action::CreateFolder => {
                if let Some(manager) = &mut self.view.folder_manager {
                    manager.editor = Some(FolderEditorView {
                        id: None,
                        title: String::new(),
                        rules: Some(FolderRulesView::default()),
                        selected: 0,
                    });
                }
                None
            }
            Action::EditFolder => {
                let details = self.view.folder_details.get(selected).copied()?;
                let title = self
                    .view
                    .folders
                    .iter()
                    .find(|folder| folder.id == details.id.0)?
                    .title
                    .clone();
                if let Some(manager) = &mut self.view.folder_manager {
                    manager.editor = Some(FolderEditorView {
                        id: Some(details.id),
                        title,
                        rules: details.rules,
                        selected: 0,
                    });
                }
                None
            }
            Action::ReorderFolderUp | Action::ReorderFolderDown => {
                let id = self.view.folder_details.get(selected)?.id;
                let position = if action == Action::ReorderFolderDown {
                    selected.saturating_add(1).min(count.saturating_sub(1))
                } else {
                    selected.saturating_sub(1)
                };
                if position == selected {
                    None
                } else {
                    Some(self.folder_effect(FolderOperation::Reorder { id, position }))
                }
            }
            Action::ShareFolder => {
                let details = *self.view.folder_details.get(selected)?;
                details
                    .shareable
                    .then(|| self.folder_effect(FolderOperation::Share { id: details.id }))
            }
            Action::DeleteFolder => {
                let id = self.view.folder_details.get(selected)?.id;
                if let Some(manager) = &mut self.view.folder_manager {
                    manager.delete_confirmation = Some(id);
                }
                None
            }
            Action::Cancel | Action::ManageFolderLifecycle => {
                self.view.folder_manager = None;
                None
            }
            _ => None,
        }
    }

    fn apply_folder_editor(&mut self, action: Action) -> Option<Effect> {
        let manager = self.view.folder_manager.as_mut()?;
        let editor = manager.editor.as_mut()?;
        match action {
            Action::MoveUp | Action::MoveDown => {
                let rows = 1 + usize::from(editor.rules.is_some()) * 8;
                editor.selected =
                    move_index(Some(editor.selected), rows, action == Action::MoveDown)
                        .unwrap_or(0);
                None
            }
            Action::ToggleFolderRule if editor.selected > 0 => {
                if let Some(rules) = &mut editor.rules {
                    rules.toggle(editor.selected - 1);
                }
                None
            }
            Action::SaveFolder if !editor.title.trim().is_empty() => {
                let operation = editor.id.map_or_else(
                    || FolderOperation::Create {
                        title: editor.title.trim().to_owned(),
                        rules: editor.rules.unwrap_or_default(),
                    },
                    |id| FolderOperation::Update {
                        id,
                        title: editor.title.trim().to_owned(),
                        rules: editor.rules,
                    },
                );
                Some(self.folder_effect(operation))
            }
            Action::Cancel => {
                manager.editor = None;
                None
            }
            _ => None,
        }
    }

    fn apply_folder_delete_confirmation(&mut self, action: Action) -> Option<Effect> {
        let manager = self.view.folder_manager.as_mut()?;
        match action {
            Action::ConfirmDeleteFolder => {
                let id = manager.delete_confirmation.take()?;
                Some(self.folder_effect(FolderOperation::Delete { id }))
            }
            Action::Cancel | Action::DeleteFolder => {
                manager.delete_confirmation = None;
                None
            }
            _ => None,
        }
    }

    fn folder_effect(&mut self, operation: FolderOperation) -> Effect {
        if let Some(manager) = &mut self.view.folder_manager {
            manager.pending = true;
        }
        Effect::FolderOperation { operation }
    }
}
