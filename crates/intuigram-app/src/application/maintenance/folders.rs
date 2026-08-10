use super::*;

pub(crate) async fn run_folder_maintenance(
    config: &Config,
    config_directory: &std::path::Path,
    account: AccountId,
    command: FolderMaintenance,
) -> Result<()> {
    let mut client = connect_account(config, config_directory, account).await?;

    match command {
        FolderMaintenance::Create { title, rules } => {
            let folder = client
                .create_folder(title, rules)
                .await
                .context(TelegramSnafu)?;
            println!("Created Telegram Folder {folder}.");
        }
        FolderMaintenance::Rename { folder, title } => {
            client
                .rename_folder(folder, title)
                .await
                .context(TelegramSnafu)?;
            println!("Renamed Telegram Folder {folder}.");
        }
        FolderMaintenance::Reorder { folder, position } => {
            client
                .reorder_folder(folder, position)
                .await
                .context(TelegramSnafu)?;
            println!("Moved Telegram Folder {folder} to position {position}.");
        }
        FolderMaintenance::Share { folder } => {
            let url = client.share_folder(folder).await.context(TelegramSnafu)?;
            println!("{url}");
        }
        FolderMaintenance::Delete { folder } => {
            println!("Delete Telegram Folder {folder}? Chats and Messages will not be deleted.");
            let confirmation = prompt(
                &format!("Type DELETE FOLDER {folder} to continue"),
                "Folder deletion confirmation",
            )?;
            if confirmation != format!("DELETE FOLDER {folder}") {
                println!("Folder was not changed.");
                return Ok(());
            }
            client.delete_folder(folder).await.context(TelegramSnafu)?;
            println!("Deleted Telegram Folder {folder}; its Chats were retained.");
        }
        FolderMaintenance::Rules { folder, rules } => {
            client
                .set_folder_rules(folder, rules)
                .await
                .context(TelegramSnafu)?;
            println!("Updated Telegram Folder {folder} inclusion and exclusion rules.");
        }
    }
    Ok(())
}
