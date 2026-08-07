use super::*;

pub(crate) async fn run_logout(
    config: &Config,
    config_directory: &std::path::Path,
    account: AccountId,
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
    println!(
        "Log out {} (Telegram user {})? Telegram must confirm revocation before Intuigram deletes \
         its local authorization, synchronized Chat and Message records, Drafts, recovery \
         backups, and Media Cache.",
        record.display_name,
        account.get()
    );
    let confirmation = prompt(
        &format!("Type LOGOUT {} to continue", account.get()),
        "logout confirmation",
    )?;
    if confirmation != format!("LOGOUT {}", account.get()) {
        println!("Account data was not changed.");
        return Ok(());
    }
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
        display_name: record.display_name.clone(),
        username: None,
    };
    let mut client =
        Client::connect_existing(credentials, &session, identity, telegram_route(config)?)
            .await
            .context(TelegramSnafu)?;
    let revoked = client.log_out().await.context(TelegramSnafu);
    drop(client);
    drop(database);
    after_verified_revocation(revoked, || {
        remove_local_account(config, layout, global, account, &record.display_name)
    })?;
    println!("Telegram revoked the authorization before local Account data was removed.");
    Ok(())
}

fn after_verified_revocation<T, E>(
    revocation: std::result::Result<(), E>,
    remove: impl FnOnce() -> std::result::Result<T, E>,
) -> std::result::Result<T, E> {
    revocation?;
    remove()
}

pub(super) fn remove_local_account(
    config: &Config,
    layout: StoreLayout,
    global: GlobalDatabase,
    account: AccountId,
    identity: &str,
) -> Result<()> {
    delete_local_lock_key(config, account).context(LocalLockSnafu)?;
    global.remove(account).context(UpdateAccountRegistrySnafu)?;
    drop(global);
    let durable = intuigram_store::AccountDataRemoval::clear(&layout, account)
        .context(ClearAccountDataSnafu)?;
    let media = intuigram_media::MediaCache::new(
        config.paths.cache.join(account.get().to_string()),
        config.media.cache_bytes,
    )
    .clear()
    .context(MediaCacheSnafu)?;
    println!(
        "Removed {} durable files and {} cached bytes for {identity}.",
        durable.removed.len(),
        media.bytes
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::after_verified_revocation;

    #[test]
    fn failed_or_offline_revocation_never_reaches_local_deletion() {
        let removed = Cell::new(false);
        let result = after_verified_revocation::<(), _>(Err("offline"), || {
            removed.set(true);
            Ok(())
        });

        assert_eq!(result, Err("offline"));
        assert!(!removed.get());
    }
}
