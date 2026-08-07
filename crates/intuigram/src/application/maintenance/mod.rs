use super::*;

mod account_exit;
mod folders;
mod media;
mod rich_media;

pub(super) use account_exit::run_logout;
pub(super) use folders::run_folder_maintenance;
pub(super) use media::run_maintenance;
pub(super) use rich_media::run_rich_media_maintenance;

async fn connect_account(
    config: &Config,
    config_directory: &std::path::Path,
    account: AccountId,
) -> Result<Client> {
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
    Client::connect_existing(
        credentials,
        &session,
        AuthorizedUser {
            id: account.get(),
            display_name: record.display_name,
            username: None,
        },
    )
    .await
    .context(TelegramSnafu)
}
