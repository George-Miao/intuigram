use super::*;

pub(super) struct CachedSession {
    pub(super) credentials: ApplicationCredentials,
    pub(super) layout: StoreLayout,
    pub(super) account: AccountRecord,
    pub(super) bootstrap: Bootstrap,
    pub(super) accounts: Vec<AccountView>,
    pub(super) storage: AdapterStorage,
}

pub(super) async fn run_cached_account<U, E>(
    terminal: &mut U,
    events: &mut E,
    session: CachedSession,
) -> Result<AccountSessionExit>
where
    U: ApplicationUi,
    E: ApplicationEvents,
{
    let CachedSession {
        credentials,
        layout,
        account,
        bootstrap,
        accounts,
        storage,
    } = session;
    let mut bootstrap = bootstrap;
    bootstrap.accounts.clone_from(&accounts);
    let mut app = App::new();
    let mut update = app.transition(Input::Adapter(AdapterEvent::Bootstrap(bootstrap)));
    let mut pending_effects = VecDeque::with_capacity(EFFECT_CAPACITY);
    let mut retained_attachments = AttachmentStore::default();
    let mut retained_media_library = MediaLibraryStore::default();
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
                Effect::Quit => return Ok(AccountSessionExit::Quit),
                Effect::AccountLifecycle { request } => {
                    return Ok(AccountSessionExit::Lifecycle(request));
                }
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
                let Ok((mut backend, mut adapter_events, peers, mut bootstrap)) = *result else {
                    unreachable!("successful connection result was checked")
                };
                bootstrap.accounts.clone_from(&accounts);
                backend
                    .attachments
                    .merge(std::mem::take(&mut retained_attachments));
                backend
                    .media_library
                    .merge(std::mem::take(&mut retained_media_library));
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
                    ApplicationExit::Quit => return Ok(AccountSessionExit::Quit),
                    ApplicationExit::Lifecycle { request, backend } => {
                        drop(backend);
                        return Ok(AccountSessionExit::Lifecycle(request));
                    }
                    ApplicationExit::Disconnected(state) => {
                        let DisconnectedApplication {
                            app: disconnected_app,
                            backend: disconnected_backend,
                            pending_effects: disconnected_effects,
                        } = *state;
                        retained_attachments.merge(disconnected_backend.attachments);
                        retained_media_library.merge(disconnected_backend.media_library);
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
