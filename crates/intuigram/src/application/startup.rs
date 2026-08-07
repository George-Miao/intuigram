pub(super) async fn run_async(arguments: Arguments) -> Result<()> {
    let defaults = platform_defaults(arguments.config.clone())?;
    let config = ConfigLoader::new(defaults)
        .with_overrides(Overrides {
            data: arguments.data,
            cache: arguments.cache,
            downloads: arguments.downloads,
            media_cache_bytes: None,
        })
        .load()
        .context(LoadConfigurationSnafu)?;
    let layout = StoreLayout::new(config.paths.data.clone());
    let global = GlobalDatabase::open(&layout).context(OpenAccountRegistrySnafu)?;
    let view_mode = match config.view.mode {
        ConfigViewMode::Default => TuiViewMode::Default,
        ConfigViewMode::Compact => TuiViewMode::Compact,
    };
    let mut terminal = TerminalUi::enter_with_mode(view_mode).context(TerminalSnafu)?;
    let mut events = TerminalEvents::new().context(TerminalSnafu)?;
    let accounts = global.accounts().context(ReadAccountRegistrySnafu)?;
    if let Some(account) = accounts.into_iter().find(|account| account.active) {
        let database = match AccountDatabase::open_recoverable(&layout, account.id)
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
            telegram_credentials(&config)?,
            layout,
            account.clone(),
            cached_bootstrap(account.display_name, cached),
            config.paths.downloads,
        )
        .await;
    }
    let (backend, mut backend_events, peers, bootstrap) =
        authorize_new_account(&telegram_credentials(&config)?, &config, &layout, &global).await?;
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

pub(super) async fn run_cached_account<U, E>(
    terminal: &mut U,
    events: &mut E,
    credentials: ApplicationCredentials,
    layout: StoreLayout,
    account: AccountRecord,
    bootstrap: Bootstrap,
    downloads: PathBuf,
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
        downloads.clone(),
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
                        downloads.clone(),
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
                            downloads.clone(),
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
use super::*;
