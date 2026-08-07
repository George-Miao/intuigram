use super::account_exit::remove_local_account;
use super::*;

pub(crate) fn run_maintenance(config: &Config, maintenance: Maintenance) -> Result<()> {
    let account = match maintenance {
        Maintenance::MediaUsage(account)
        | Maintenance::ClearMedia(account)
        | Maintenance::ClearAccount(account)
        | Maintenance::Logout(account)
        | Maintenance::Folder(account, _)
        | Maintenance::RichMedia(account, _)
        | Maintenance::Scheduled(account, _) => account,
    };
    let cache = intuigram_media::MediaCache::new(
        config.paths.cache.join(account.get().to_string()),
        config.media.cache_bytes,
    );
    match maintenance {
        Maintenance::MediaUsage(_) => {
            let usage = cache.usage().context(MediaCacheSnafu)?;
            println!(
                "Account {} Media Cache: {} bytes in {} entries (limit {} bytes)",
                account.get(),
                usage.bytes,
                usage.entries,
                usage.limit
            );
        }
        Maintenance::ClearMedia(_) => {
            let removed = cache.clear().context(MediaCacheSnafu)?;
            println!(
                "Cleared {} bytes in {} redownloadable media entries for Account {}. Chat and \
                 Message text were retained.",
                removed.bytes,
                removed.entries,
                account.get()
            );
        }
        Maintenance::ClearAccount(_) => {
            let layout = StoreLayout::new(config.paths.data.clone());
            let global = GlobalDatabase::open(&layout).context(OpenAccountRegistrySnafu)?;
            let identity = global
                .accounts()
                .context(ReadAccountRegistrySnafu)?
                .into_iter()
                .find(|candidate| candidate.id == account)
                .map_or_else(
                    || format!("Account {}", account.get()),
                    |record| record.display_name,
                );
            println!(
                "Clear local data for {identity} (Telegram user {})? This deletes its \
                 authorization, synchronized Chat and Message records, Drafts, recovery backups, \
                 and Media Cache. The server-side Telegram authorization may remain active.",
                account.get()
            );
            let confirmation = prompt(
                &format!("Type CLEAR {} to continue", account.get()),
                "clear-account confirmation",
            )?;
            if confirmation != format!("CLEAR {}", account.get()) {
                println!("Account data was not changed.");
                return Ok(());
            }
            remove_local_account(config, layout, global, account, &identity)?;
        }
        Maintenance::Logout(_) => unreachable!("logout is handled asynchronously"),
        Maintenance::Folder(..) => unreachable!("Folder maintenance is handled asynchronously"),
        Maintenance::RichMedia(..) => {
            unreachable!("rich media maintenance is handled asynchronously")
        }
        Maintenance::Scheduled(..) => {
            unreachable!("Scheduled Message maintenance is handled asynchronously")
        }
    }
    Ok(())
}
