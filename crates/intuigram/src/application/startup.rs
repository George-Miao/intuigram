use super::*;

pub(super) async fn run_async(arguments: Arguments) -> Result<()> {
    let test_connection = arguments.test_connection;
    let defaults = platform_defaults(arguments.config.clone())?;
    let config_directory = defaults.config.clone();
    let config = ConfigLoader::new(defaults)
        .with_overrides(Overrides {
            data: arguments.data,
            cache: arguments.cache,
            downloads: arguments.downloads,
            media_cache_bytes: None,
        })
        .load()
        .context(LoadConfigurationSnafu)?;
    if test_connection {
        let route = telegram_route(&config)?;
        let credentials = resolve_telegram_credentials(&config, &config_directory)?;
        Client::test_connection(PRIMARY_DC_ID, PRIMARY_DC_ENDPOINT, credentials, route)
            .await
            .context(ProxyConnectionTestSnafu)?;
        println!("Telegram connection route completed MTProto initialization.");
        return Ok(());
    }
    if let Some(maintenance) = arguments.maintenance {
        return match maintenance {
            Maintenance::Logout(account) => run_logout(&config, &config_directory, account).await,
            Maintenance::Folder(account, command) => {
                run_folder_maintenance(&config, &config_directory, account, command).await
            }
            Maintenance::RichMedia(account, command) => {
                run_rich_media_maintenance(&config, &config_directory, account, command).await
            }
            Maintenance::Scheduled(account, command) => {
                run_scheduled_maintenance(&config, &config_directory, account, command).await
            }
            maintenance => run_maintenance(&config, maintenance),
        };
    }
    let layout = StoreLayout::new(config.paths.data.clone());
    let global = GlobalDatabase::open(&layout).context(OpenAccountRegistrySnafu)?;
    let accounts = global.accounts().context(ReadAccountRegistrySnafu)?;
    if arguments.list_accounts {
        for account in &accounts {
            println!(
                "{}\t{}{}",
                account.id.get(),
                account.display_name,
                if account.active { "\tactive" } else { "" }
            );
        }
        return Ok(());
    }
    let mut requested_account = if arguments.add_account {
        None
    } else if let Some(requested) = arguments.account {
        let selected = accounts
            .iter()
            .find(|account| account.id == requested)
            .cloned()
            .context(UnknownAccountSnafu {
                account: requested.get(),
            })?;
        global
            .register(AccountRecord {
                active: true,
                ..selected.clone()
            })
            .context(UpdateAccountRegistrySnafu)?;
        Some(selected.id)
    } else {
        accounts
            .iter()
            .find(|account| account.active)
            .map(|account| account.id)
    };
    drop(global);
    let credentials = resolve_telegram_credentials(&config, &config_directory)?;
    let view_mode = match config.view.mode {
        ConfigViewMode::Default => TuiViewMode::Default,
        ConfigViewMode::Compact => TuiViewMode::Compact,
    };
    loop {
        let global = GlobalDatabase::open(&layout).context(OpenAccountRegistrySnafu)?;
        let accounts = global.accounts().context(ReadAccountRegistrySnafu)?;
        let active_account = requested_account.and_then(|requested| {
            accounts
                .iter()
                .find(|account| account.id == requested)
                .cloned()
        });
        if let Some(account) = &active_account {
            global
                .register(AccountRecord {
                    active: true,
                    ..account.clone()
                })
                .context(UpdateAccountRegistrySnafu)?;
        }
        let registered_accounts =
            account_views(&accounts, active_account.as_ref().map(|item| item.id));
        let initializing_lock = config.local_lock.enabled
            && active_account
                .as_ref()
                .is_none_or(|account| !intuigram_store::local_lock_is_enabled(&layout, account.id));
        let mut unlock = unlock_local_lock(
            &config,
            active_account.as_ref().map(|account| account.id),
            initializing_lock,
        )
        .context(LocalLockSnafu)?;
        if active_account.is_none() {
            let account =
                authorize_new_account(&credentials, &config, &layout, &global, unlock.cipher())
                    .await?;
            unlock.promote(&config, account).context(LocalLockSnafu)?;
            requested_account = Some(account);
            continue;
        }
        let account = active_account.expect("new Account authorization continues the outer loop");
        let mut terminal = TerminalUi::enter_with_mode(view_mode).context(TerminalSnafu)?;
        let mut events = TerminalEvents::new().context(TerminalSnafu)?;
        if unlock.cipher().is_encrypted() {
            intuigram_store::enable_local_lock(&layout, account.id, &unlock.cipher())
                .context(EnableLocalLockSnafu)?;
        }
        let database = match AccountDatabase::open_recoverable_with_cipher(
            &layout,
            account.id,
            unlock.cipher(),
        )
        .context(AccountDatabaseSnafu)?
        {
            AccountOpen::Ready(database) => database,
            AccountOpen::Recovery(recovery) => match crate::recovery::run(
                &mut terminal,
                &mut events,
                recovery,
                account.display_name.clone(),
            )
            .await
            .context(AccountRecoverySnafu)?
            {
                crate::recovery::Outcome::Ready(database) => database,
                crate::recovery::Outcome::Cancelled => return Ok(()),
            },
        };
        let cached = database.cached_account().context(AccountDatabaseSnafu)?;
        drop(database);
        unlock
            .promote(&config, account.id)
            .context(LocalLockSnafu)?;
        let outcome = run_cached_account(
            &mut terminal,
            &mut events,
            CachedSession {
                credentials: credentials.clone(),
                layout: layout.clone(),
                account: account.clone(),
                bootstrap: cached_bootstrap(
                    account.display_name.clone(),
                    account.notification_identity.clone(),
                    cached,
                ),
                accounts: registered_accounts,
                storage: AdapterStorage {
                    downloads: config.paths.downloads.clone(),
                    cache_root: config.paths.cache.clone(),
                    cache_limit: config.media.cache_bytes,
                    cipher: unlock.cipher(),
                    route: telegram_route(&config)?,
                },
            },
        )
        .await?;
        drop(events);
        drop(terminal);
        match outcome {
            AccountSessionExit::Quit => return Ok(()),
            AccountSessionExit::Lifecycle(AccountLifecycle::Add) => requested_account = None,
            AccountSessionExit::Lifecycle(AccountLifecycle::Switch(key)) => {
                requested_account = Some(account_id(key)?);
            }
            AccountSessionExit::Lifecycle(
                AccountLifecycle::Logout(key) | AccountLifecycle::RemoveLocal(key),
            ) => {
                let account = account_id(key)?;
                let fallback = accounts
                    .iter()
                    .find(|candidate| candidate.id != account)
                    .map(|candidate| candidate.id);
                let record = accounts
                    .iter()
                    .find(|candidate| candidate.id == account)
                    .context(UnknownAccountSnafu {
                        account: account.get(),
                    })?;
                remove_local_account(
                    &config,
                    layout.clone(),
                    global,
                    account,
                    &record.display_name,
                )?;
                requested_account = fallback;
            }
        }
    }
}

fn account_id(key: AccountKey) -> Result<AccountId> {
    AccountId::new(key.0).context(InvalidAccountIdSnafu { value: key.0 })
}

fn account_views(accounts: &[AccountRecord], active: Option<AccountId>) -> Vec<AccountView> {
    accounts
        .iter()
        .map(|account| AccountView {
            id: AccountKey(account.id.get()),
            display_name: account.display_name.clone(),
            active: Some(account.id) == active,
        })
        .collect()
}
