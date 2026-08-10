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
    let mut retained = RetainedBackend::default();
    let mut attempt = Some(ActorSession::connection(
        credentials.clone(),
        layout.clone(),
        account.clone(),
        storage.clone(),
    ));
    let mut animation_timer: Option<Pin<Box<dyn Future<Output = ()>>>> = None;

    loop {
        terminal.draw(&update.view).context(TerminalSnafu)?;
        if let Some(effect) = update.effect.take() {
            match effect {
                Effect::Quit => {
                    cancel_connection(&mut attempt).await?;
                    return Ok(AccountSessionExit::Quit);
                }
                Effect::AccountLifecycle { request } => {
                    cancel_connection(&mut attempt).await?;
                    return Ok(AccountSessionExit::Lifecycle(request));
                }
                Effect::Reconnect if attempt.is_none() => {
                    attempt = Some(ActorSession::connection(
                        credentials.clone(),
                        layout.clone(),
                        account.clone(),
                        storage.clone(),
                    ));
                }
                Effect::Reconnect => {}
                effect => {
                    enqueue_effect(
                        &mut pending_effects,
                        &futures_util::stream::FuturesUnordered::new(),
                        &[],
                        Some(effect),
                    )?;
                }
            }
        }

        enum Wake<T> {
            Terminal(T),
            Redraw(intuigram_tui::Result<()>),
            Connected(Box<Result<ConnectedActorSession>>),
            Animate,
        }
        if update.view.has_pending_effort() {
            if animation_timer.is_none() {
                animation_timer = Some(Box::pin(compio::time::sleep(Duration::from_millis(90))));
            }
        } else {
            animation_timer = None;
        }
        let wake = poll_fn(|cx| {
            if let Poll::Ready(result) = terminal.poll_redraw(cx) {
                return Poll::Ready(Wake::Redraw(result));
            }
            if let Poll::Ready(event) = events.poll_next_event(cx) {
                return Poll::Ready(Wake::Terminal(event));
            }
            if let Some(connection) = &mut attempt
                && let Poll::Ready(result) = Pin::new(connection).poll(cx)
            {
                return Poll::Ready(Wake::Connected(Box::new(result)));
            }
            if animation_timer
                .as_mut()
                .is_some_and(|timer| timer.as_mut().poll(cx).is_ready())
            {
                return Poll::Ready(Wake::Animate);
            }
            Poll::Pending
        })
        .await;

        match wake {
            Wake::Redraw(result) => result.context(TerminalSnafu)?,
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
            Wake::Animate => {
                animation_timer = None;
                update = app.transition(Input::Intent(Intent::Animate));
            }
            Wake::Connected(result) if result.is_ok() => {
                let Ok(ConnectedActorSession {
                    backend,
                    events: mut adapter_events,
                    peers,
                    mut bootstrap,
                }) = *result
                else {
                    unreachable!("successful connection result was checked")
                };
                backend.restore_retained(std::mem::take(&mut retained))?;
                bootstrap.accounts.clone_from(&accounts);
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
                        backend.shutdown().await?;
                        return Ok(AccountSessionExit::Lifecycle(request));
                    }
                    ApplicationExit::Disconnected(state) => {
                        let DisconnectedApplication {
                            app: disconnected_app,
                            backend: disconnected_backend,
                            pending_effects: disconnected_effects,
                        } = *state;
                        retained = disconnected_backend.take_retained().await?;
                        disconnected_backend.shutdown().await?;
                        app = disconnected_app;
                        pending_effects = disconnected_effects;
                        update = app.transition(Input::Adapter(AdapterEvent::ConnectionChanged(
                            ConnectionState::Connecting,
                        )));
                        attempt = Some(ActorSession::connection(
                            credentials.clone(),
                            layout.clone(),
                            account.clone(),
                            storage.clone(),
                        ));
                    }
                }
            }
            Wake::Connected(result) => {
                attempt = None;
                let Err(error) = *result else {
                    unreachable!("failed connection result was checked")
                };
                let reason = error_lines(&error).join(": ");
                update = app.transition(Input::Adapter(AdapterEvent::ConnectionFailed(reason)));
            }
        }
    }
}

async fn cancel_connection(attempt: &mut Option<ActorConnection>) -> Result<()> {
    match attempt.take() {
        Some(connection) => connection.cancel().await,
        None => Ok(()),
    }
}
