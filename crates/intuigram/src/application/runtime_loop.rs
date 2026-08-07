pub(super) async fn run_application<U, E, A, B>(
    terminal: &mut U,
    events: &mut E,
    adapter_events: &mut A,
    backend: B,
    peers: intuigram_telegram::PeerDirectory,
    bootstrap: Bootstrap,
) -> Result<ApplicationExit<B>>
where
    U: ApplicationUi,
    E: ApplicationEvents,
    A: ApplicationAdapterEvents,
    B: ApplicationBackend,
{
    let (app, update) = Application::new(bootstrap).into_parts();
    run_application_state(
        terminal,
        events,
        adapter_events,
        backend,
        ApplicationState {
            app,
            update,
            pending_effects: VecDeque::with_capacity(EFFECT_CAPACITY),
            peers,
        },
    )
    .await
}

pub(super) async fn run_application_state<U, E, A, B>(
    terminal: &mut U,
    events: &mut E,
    adapter_events: &mut A,
    backend: B,
    state: ApplicationState,
) -> Result<ApplicationExit<B>>
where
    U: ApplicationUi,
    E: ApplicationEvents,
    A: ApplicationAdapterEvents,
    B: ApplicationBackend,
{
    let ApplicationState {
        mut app,
        mut update,
        mut pending_effects,
        mut peers,
    } = state;
    let mut backend = Some(backend);
    let mut active_effect: Option<PendingEffect<B>> = None;
    let mut animation_timer: Option<Pin<Box<dyn Future<Output = ()>>>> = None;
    let mut disconnected = false;

    loop {
        terminal.draw(&update.view).context(TerminalSnafu)?;
        if enqueue_effect(&mut pending_effects, &active_effect, update.effect.take())? {
            return Ok(ApplicationExit::Quit);
        }

        if disconnected && active_effect.is_none() {
            return Ok(ApplicationExit::Disconnected(Box::new(
                DisconnectedApplication {
                    app,
                    backend: backend
                        .take()
                        .expect("completed effects return the disconnected backend"),
                    pending_effects,
                },
            )));
        }

        if !disconnected
            && active_effect.is_none()
            && let Some(effect) = pending_effects.pop_front()
        {
            let available = backend
                .take()
                .expect("backend is available whenever no effect owns it");
            active_effect = Some(start_effect(available, effect, peers.clone()));
        }

        if update.view.has_pending_effort() {
            if animation_timer.is_none() {
                animation_timer = Some(Box::pin(compio::time::sleep(Duration::from_millis(90))));
            }
        } else {
            animation_timer = None;
        }

        let wake = poll_fn(|cx| {
            if !disconnected && let Poll::Ready(event) = adapter_events.poll_adapter_event(cx) {
                return Poll::Ready(ApplicationWake::Adapter(event));
            }
            if let Poll::Ready(event) = events.poll_next_event(cx) {
                return Poll::Ready(ApplicationWake::Terminal(event));
            }
            if let Some(effect) = &mut active_effect
                && let Poll::Ready(completion) = effect.as_mut().poll(cx)
            {
                return Poll::Ready(ApplicationWake::Backend(completion));
            }
            if let Some(timer) = &mut animation_timer
                && let Poll::Ready(()) = timer.as_mut().poll(cx)
            {
                return Poll::Ready(ApplicationWake::Animation);
            }
            Poll::Pending
        })
        .await;

        match wake {
            ApplicationWake::Terminal(event) => {
                let event = event.context(TerminalSnafu)?;
                let Some(event) = terminal.resolve_event(&update.view, event) else {
                    continue;
                };
                match event {
                    UiEvent::Redraw => {}
                    UiEvent::Intent(intent) => {
                        update = app.transition(Input::Intent(intent));
                    }
                }
            }
            ApplicationWake::Adapter(event) => match event {
                Ok(batch) => {
                    peers.merge(batch.peers);
                    update = match batch.event {
                        Some(event) => app.transition(Input::Adapter(event)),
                        None => Update {
                            view: app.view(),
                            effect: None,
                        },
                    };
                }
                Err(error) => {
                    let Some(reason) = connection_failure_reason(&error) else {
                        return Err(error);
                    };
                    disconnected = true;
                    update = app.transition(Input::Adapter(AdapterEvent::ConnectionFailed(reason)));
                }
            },
            ApplicationWake::Backend(completion) => {
                active_effect = None;
                backend = Some(completion.backend);
                match completion.result {
                    Ok(output) => {
                        if let Some(returned) = output.telegram_update {
                            adapter_events.submit_update(returned);
                        }
                        if let Some(AdapterEvent::AccountLifecycleReady(request)) =
                            output.event.as_ref()
                        {
                            return Ok(ApplicationExit::Lifecycle {
                                request: *request,
                                backend: backend
                                    .take()
                                    .expect("the completed lifecycle effect returned its backend"),
                            });
                        }
                        update = match output.event {
                            Some(event) => app.transition(Input::Adapter(event)),
                            None => Update {
                                view: app.view(),
                                effect: None,
                            },
                        };
                    }
                    Err(error) => {
                        let Some(reason) = connection_failure_reason(&error) else {
                            return Err(error);
                        };
                        pending_effects.push_front(completion.effect);
                        disconnected = true;
                        update =
                            app.transition(Input::Adapter(AdapterEvent::ConnectionFailed(reason)));
                    }
                }
            }
            ApplicationWake::Animation => {
                animation_timer = None;
                update = app.transition(Input::Intent(Intent::Animate));
            }
        }
    }
}
use super::*;
