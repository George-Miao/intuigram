use super::*;

pub(super) async fn run_async(arguments: Arguments) -> Result<()> {
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
    if let Some(maintenance) = arguments.maintenance {
        return match maintenance {
            Maintenance::Logout(account) => run_logout(&config, &config_directory, account).await,
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
    let active_account = if arguments.add_account {
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
        Some(selected)
    } else {
        accounts.iter().find(|account| account.active).cloned()
    };
    let credentials = resolve_telegram_credentials(&config, &config_directory)?;
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
    let view_mode = match config.view.mode {
        ConfigViewMode::Default => TuiViewMode::Default,
        ConfigViewMode::Compact => TuiViewMode::Compact,
    };
    let mut terminal = TerminalUi::enter_with_mode(view_mode).context(TerminalSnafu)?;
    let mut events = TerminalEvents::new().context(TerminalSnafu)?;
    if let Some(account) = active_account {
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
            AccountOpen::Recovery(recovery) => {
                match crate::recovery::run(
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
                }
            }
        };
        let cached = database.cached_account().context(AccountDatabaseSnafu)?;
        drop(database);
        return run_cached_account(
            &mut terminal,
            &mut events,
            credentials,
            layout,
            account.clone(),
            cached_bootstrap(account.display_name, cached),
            AdapterStorage {
                downloads: config.paths.downloads,
                cache_root: config.paths.cache,
                cache_limit: config.media.cache_bytes,
                cipher: unlock.cipher(),
            },
        )
        .await;
    }
    let (backend, mut backend_events, peers, bootstrap) =
        authorize_new_account(&credentials, &config, &layout, &global, unlock.cipher()).await?;
    let account = backend
        ._database
        .account_id()
        .context(AccountDatabaseSnafu)?
        .expect("an authorized backend always has a persisted Account identity");
    unlock.promote_keyring(account).context(LocalLockSnafu)?;
    run_application(
        &mut terminal,
        &mut events,
        &mut backend_events,
        backend,
        peers,
        bootstrap,
    )
    .await
}

fn run_maintenance(config: &Config, maintenance: Maintenance) -> Result<()> {
    let account = match maintenance {
        Maintenance::MediaUsage(account)
        | Maintenance::ClearMedia(account)
        | Maintenance::ClearAccount(account)
        | Maintenance::Logout(account) => account,
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
    }
    Ok(())
}

async fn run_logout(
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
    let mut client = Client::connect_existing(credentials, &session, identity)
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

fn remove_local_account(
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

pub(super) async fn run_cached_account<U, E>(
    terminal: &mut U,
    events: &mut E,
    credentials: ApplicationCredentials,
    layout: StoreLayout,
    account: AccountRecord,
    bootstrap: Bootstrap,
    storage: AdapterStorage,
) -> Result<()>
where
    U: ApplicationUi,
    E: ApplicationEvents,
{
    let mut app = App::new();
    let mut update = app.transition(Input::Adapter(AdapterEvent::Bootstrap(bootstrap)));
    let mut pending_effects = VecDeque::with_capacity(EFFECT_CAPACITY);
    let mut retained_attachments = AttachmentStore::default();
    let mut retained_downloads = DownloadStore::default();
    let mut attempt = Some(Box::pin(resume_account(
        credentials.clone(),
        &layout,
        &account,
        storage.clone(),
    )));

    loop {
        terminal.draw(&update.view).context(TerminalSnafu)?;
        if let Some(effect) = update.effect.take() {
            match effect {
                Effect::Quit => return Ok(()),
                Effect::Reconnect if attempt.is_none() => {
                    attempt = Some(Box::pin(resume_account(
                        credentials.clone(),
                        &layout,
                        &account,
                        storage.clone(),
                    )));
                }
                Effect::Reconnect => {}
                effect => {
                    enqueue_effect::<Backend>(&mut pending_effects, &None, Some(effect))?;
                }
            }
        }

        enum Wake<T> {
            Terminal(T),
            Connected(
                Box<
                    Result<(
                        Backend,
                        BackendEvents,
                        intuigram_telegram::PeerDirectory,
                        Bootstrap,
                    )>,
                >,
            ),
        }
        let wake = poll_fn(|cx| {
            if let Poll::Ready(event) = events.poll_next_event(cx) {
                return Poll::Ready(Wake::Terminal(event));
            }
            if let Some(connection) = &mut attempt
                && let Poll::Ready(result) = connection.as_mut().poll(cx)
            {
                return Poll::Ready(Wake::Connected(Box::new(result)));
            }
            Poll::Pending
        })
        .await;

        match wake {
            Wake::Terminal(event) => {
                let event = event.context(TerminalSnafu)?;
                let Some(event) = terminal.resolve_event(&update.view, event) else {
                    continue;
                };
                match event {
                    UiEvent::Redraw => {}
                    UiEvent::Intent(intent) => update = app.transition(Input::Intent(intent)),
                }
            }
            Wake::Connected(result) if result.is_ok() => {
                let Ok((mut backend, mut adapter_events, peers, bootstrap)) = *result else {
                    unreachable!("successful connection result was checked")
                };
                backend
                    .attachments
                    .merge(std::mem::take(&mut retained_attachments));
                backend
                    .downloaded
                    .merge(std::mem::take(&mut retained_downloads));
                update =
                    app.transition(Input::Adapter(AdapterEvent::ConnectionRestored(bootstrap)));
                match run_application_state(
                    terminal,
                    events,
                    &mut adapter_events,
                    backend,
                    ApplicationState {
                        app,
                        update,
                        pending_effects,
                        peers,
                    },
                )
                .await?
                {
                    ApplicationExit::Quit => return Ok(()),
                    ApplicationExit::Disconnected(state) => {
                        let DisconnectedApplication {
                            app: disconnected_app,
                            backend: disconnected_backend,
                            pending_effects: disconnected_effects,
                        } = *state;
                        retained_attachments.merge(disconnected_backend.attachments);
                        retained_downloads.merge(disconnected_backend.downloaded);
                        app = disconnected_app;
                        pending_effects = disconnected_effects;
                        update = app.transition(Input::Adapter(AdapterEvent::ConnectionChanged(
                            ConnectionState::Connecting,
                        )));
                        attempt = Some(Box::pin(resume_account(
                            credentials.clone(),
                            &layout,
                            &account,
                            storage.clone(),
                        )));
                    }
                }
            }
            Wake::Connected(result) => {
                let Err(error) = *result else {
                    unreachable!("failed connection result was checked")
                };
                attempt = None;
                let reason = error_lines(&error).join(": ");
                update = app.transition(Input::Adapter(AdapterEvent::ConnectionFailed(reason)));
            }
        }
    }
}

#[cfg(test)]
mod exit_tests {
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
