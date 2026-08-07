use super::*;

impl Backend {
    pub(super) async fn execute_folder_operation(
        &mut self,
        operation: FolderOperation,
    ) -> Result<AdapterEvent> {
        let result = match operation {
            FolderOperation::Create { title, rules } => self
                .client
                .create_folder(title.clone(), rules.into())
                .await
                .map(|id| FolderOperationResult::Created { id, title, rules }),
            FolderOperation::Update { id, title, rules } => self
                .client
                .update_folder_settings(id, title.clone(), rules.map(Into::into))
                .await
                .map(|()| FolderOperationResult::Updated { id, title, rules }),
            FolderOperation::Reorder { id, position } => self
                .client
                .reorder_folder(id, position)
                .await
                .map(|()| FolderOperationResult::Reordered { id, position }),
            FolderOperation::Share { id } => self
                .client
                .share_folder(id)
                .await
                .map(|url| FolderOperationResult::Shared { id, url }),
            FolderOperation::Delete { id } => self
                .client
                .delete_folder(id)
                .await
                .map(|()| FolderOperationResult::Deleted { id }),
        };
        match result {
            Ok(result) => {
                let reconciliation = if matches!(&result, FolderOperationResult::Shared { .. }) {
                    None
                } else {
                    self.client.bootstrap(100).await.ok().map(|bootstrap| {
                        Box::new(intuigram_app::FolderReconciliation {
                            folders: bootstrap.folders,
                            details: bootstrap.folder_details,
                            chats: bootstrap.chats,
                        })
                    })
                };
                Ok(AdapterEvent::FolderOperationCompleted {
                    result,
                    reconciliation,
                })
            }
            Err(source) if source.is_connection_failure() => Err(Error::Telegram { source }),
            Err(error) => Ok(AdapterEvent::FolderOperationFailed(error.to_string())),
        }
    }
}
