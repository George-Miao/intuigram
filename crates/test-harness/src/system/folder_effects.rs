use intuigram_app::{
    AdapterEvent, FolderId, FolderOperation, FolderOperationResult, FolderReconciliation,
};

use super::TestSystem;

impl TestSystem {
    pub(super) fn handle_folder_operation(&mut self, operation: FolderOperation) {
        let result = match operation {
            FolderOperation::Create { title, rules } => {
                let id = self
                    .application
                    .view()
                    .folder_details
                    .iter()
                    .map(|details| details.id.0)
                    .max()
                    .unwrap_or(1)
                    .saturating_add(1);
                FolderOperationResult::Created {
                    id: FolderId(id),
                    title,
                    rules,
                }
            }
            FolderOperation::Update { id, title, rules } => {
                FolderOperationResult::Updated { id, title, rules }
            }
            FolderOperation::Reorder { id, position } => {
                FolderOperationResult::Reordered { id, position }
            }
            FolderOperation::Share { id } => FolderOperationResult::Shared {
                id,
                url: format!("https://t.me/addlist/folder-{}", id.0),
            },
            FolderOperation::Delete { id } => FolderOperationResult::Deleted { id },
        };
        self.application
            .handle_adapter(AdapterEvent::FolderOperationCompleted {
                result,
                reconciliation: None,
            });
    }

    pub(super) fn handle_folder_refresh(&mut self) {
        let view = self.application.view();
        self.application
            .handle_adapter(AdapterEvent::FolderReconciled(Box::new(
                FolderReconciliation {
                    folders: view.folders.clone(),
                    details: view.folder_details.clone(),
                    chats: view.chats.clone(),
                },
            )));
    }
}
