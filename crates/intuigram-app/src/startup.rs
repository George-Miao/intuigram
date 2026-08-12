use super::*;

pub(super) async fn run_async(arguments: Arguments) -> Result<()> {
    let Arguments { global, command } = arguments;
    let Global {
        config: config_path,
        data,
        cache,
        downloads,
        account,
        test_connection,
    } = global;
    let defaults = platform_defaults(config_path.clone())?;
    let config_directory = defaults.config.clone();
    let config = ConfigLoader::new(defaults)
        .with_overrides(Overrides {
            data,
            cache,
            downloads,
            media_cache_bytes: None,
        })
        .load()
        .context(LoadConfigurationSnafu)?;
    logging::initialize(&config.log_path()).context(InitializeLoggingSnafu)?;
    if test_connection {
        let route = telegram_route(&config)?;
        let credentials = resolve_telegram_credentials(&config, &config_directory)?;
        Client::test_connection(PRIMARY_DC_ID, PRIMARY_DC_ENDPOINT, credentials, route)
            .await
            .context(ProxyConnectionTestSnafu)?;
        println!("Telegram connection route completed MTProto initialization.");
        return Ok(());
    }
    let command = match command {
        Command::Maintenance(command) => {
            let account = account.expect(
                "validated launch arguments require --account for every maintenance command",
            );
            return match command.into_inner() {
                Maintenance::Logout => run_logout(&config, &config_directory, account).await,
                Maintenance::Folder(command) => {
                    run_folder_maintenance(&config, &config_directory, account, command).await
                }
                Maintenance::RichMedia(command) => {
                    run_rich_media_maintenance(&config, &config_directory, account, command).await
                }
                Maintenance::Scheduled(command) => {
                    run_scheduled_maintenance(&config, &config_directory, account, command).await
                }
                maintenance => run_maintenance(&config, account, maintenance),
            };
        }
        command => command,
    };
    let layout = StoreLayout::new(config.paths.data.clone());
    let global = GlobalDatabase::open(&layout).context(OpenAccountRegistrySnafu)?;
    let accounts = global.accounts().context(ReadAccountRegistrySnafu)?;
    if matches!(&command, Command::AccountList) {
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
    let mut requested_account = match command {
        Command::AccountAdd => None,
        Command::Start => {
            if let Some(requested) = account {
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
            }
        }
        Command::AccountList | Command::Maintenance(_) => {
            unreachable!("handled launch command reached Account selection")
        }
    };
    drop(global);
    let credentials = resolve_telegram_credentials(&config, &config_directory)?;
    let mode = match config.view.mode {
        ConfigViewMode::Default => TuiViewMode::Default,
        ConfigViewMode::Compact => TuiViewMode::Compact,
    };
    let view_options = TuiViewOptions {
        mode,
        message_max_width: config.view.message_max_width,
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
        let mut terminal = TerminalUi::enter_with_options(view_options).context(TerminalSnafu)?;
        let mut events = TerminalEvents::new().context(TerminalSnafu)?;
        let mut prepared = wait_for_account_load(
            &mut terminal,
            &mut events,
            account.display_name.clone(),
            account.notification_identity.clone(),
            registered_accounts.clone(),
            prepare_account(
                layout.clone(),
                account.clone(),
                unlock.cipher(),
                registered_accounts.clone(),
            ),
        )
        .await
        .context(AccountLoadingSnafu)?;
        let bootstrap = loop {
            match prepared {
                Loading::Exit(outcome) => break Err(outcome),
                Loading::Ready(result) => match result.context(AccountLoadingSnafu)? {
                    PreparedAccount::Ready(bootstrap) => break Ok(*bootstrap),
                    PreparedAccount::Recovery(recovery) => {
                        let database = match crate::recovery::run(
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
                        };
                        prepared = wait_for_account_load(
                            &mut terminal,
                            &mut events,
                            account.display_name.clone(),
                            account.notification_identity.clone(),
                            registered_accounts.clone(),
                            prepare_recovered(
                                database,
                                account.clone(),
                                registered_accounts.clone(),
                            ),
                        )
                        .await
                        .context(AccountLoadingSnafu)?;
                    }
                },
            }
        };
        let outcome = match bootstrap {
            Err(outcome) => outcome,
            Ok(bootstrap) => {
                unlock
                    .promote(&config, account.id)
                    .context(LocalLockSnafu)?;
                run_cached_account(
                    &mut terminal,
                    &mut events,
                    CachedSession {
                        credentials: credentials.clone(),
                        layout: layout.clone(),
                        account: account.clone(),
                        bootstrap,
                        accounts: registered_accounts,
                        storage: AdapterStorage {
                            downloads: config.paths.downloads.clone(),
                            cache_root: config.paths.cache.clone(),
                            cache_limit: config.media.cache_bytes,
                            cipher: unlock.cipher(),
                            route: telegram_route(&config)?,
                            path_picker: config.external.path_picker.clone(),
                        },
                    },
                )
                .await?
            }
        };
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
