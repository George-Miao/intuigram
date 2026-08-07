use super::*;

pub(crate) async fn run_folder_maintenance(
    config: &Config,
    config_directory: &std::path::Path,
    account: AccountId,
    command: FolderMaintenance,
) -> Result<()> {
    let layout = StoreLayout::new(config.paths.data.clone());
    let global = GlobalDatabase::open(&layout).context(OpenAccountRegistrySnafu)?;
    let record = global
        .accounts()
        .context(ReadAccountRegistrySnafu)?
        .into_iter()
        .find(|candidate| candidate.id == account)
        .context(UnknownAccountSnafu {
            account: account.get(),
        })?;
    let credentials = resolve_telegram_credentials(config, config_directory)?;
    let unlock = unlock_local_lock(config, Some(account), false).context(LocalLockSnafu)?;
    if unlock.cipher().is_encrypted() {
        intuigram_store::enable_local_lock(&layout, account, &unlock.cipher())
            .context(EnableLocalLockSnafu)?;
    }
    let database = AccountDatabase::open_with_cipher(&layout, account, unlock.cipher())
        .context(AccountDatabaseSnafu)?;
    let stored = database
        .session()
        .context(AccountDatabaseSnafu)?
        .context(MissingSessionSnafu { account })?;
    let session = telegram_session(&stored)?;
    let identity = AuthorizedUser {
        id: account.get(),
        display_name: record.display_name,
        username: None,
    };
    let mut client = Client::connect_existing(credentials, &session, identity)
        .await
        .context(TelegramSnafu)?;

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
