use super::*;

mod adapters;
mod effect_route;
mod pending_operation;
mod types;
mod wake_poll;

#[cfg(test)]
pub(super) use adapters::WorkerBatch;
pub(super) use adapters::{
    AdapterBatch, ApplicationAdapterEvents, ApplicationBackend, ApplicationEvents, BackendOutput,
    WorkerAdapterEvents,
};
pub(super) use effect_route::{EffectRoute, effect_route};
pub(super) use pending_operation::wait_for_reconnect_cleanup;
#[cfg(test)]
pub(super) use types::PendingEffect;
pub(super) use types::{
    AccountSessionExit, AdapterEffect, ApplicationExit, ApplicationState, ApplicationWake,
    DisconnectedApplication, connection_failure_reason, enqueue_effect, notification_key,
    start_effect,
};
use wake_poll::{WakePolicy, WakePoller, WakeSources};

#[cfg(test)]
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
    let mut active_effects = futures_util::stream::FuturesUnordered::new();
    let mut animation_timer: Option<Pin<Box<dyn Future<Output = ()>>>> = None;
    let mut wake_poller = WakePoller::new();
    let mut disconnected = false;
    let mut telegram_effect_active = false;
    let mut storage_effect_active = false;
    let mut active_notifications = Vec::new();
    let mut requested_exit = None;
    let mut stopping_error = None;

    loop {
        terminal.draw(&update.view).context(TerminalSnafu)?;
        if requested_exit.is_none()
            && enqueue_effect(
                &mut pending_effects,
                &active_effects,
                &active_notifications,
                update.effect.take(),
            )?
        {
            requested_exit = Some(RequestedExit::Quit);
            pending_effects.clear();
            backend.begin_shutdown();
        }

        if pending_effects.is_empty()
            && active_effects.is_empty()
            && let Some(requested_exit) = requested_exit
        {
            if let Some(error) = stopping_error {
                backend.shutdown().await?;
                return Err(error);
            }
            return match requested_exit {
                RequestedExit::Quit => {
                    backend.shutdown().await?;
                    Ok(ApplicationExit::Quit)
                }
                RequestedExit::Lifecycle(request) => {
                    Ok(ApplicationExit::Lifecycle { request, backend })
                }
            };
        }

        if disconnected && active_effects.is_empty() {
            return Ok(ApplicationExit::Disconnected(Box::new(
                DisconnectedApplication {
                    app,
                    backend,
                    pending_effects,
                },
            )));
        }

        while requested_exit.is_none() && !disconnected && active_effects.len() < EFFECT_CAPACITY {
            let position =
                pending_effects
                    .iter()
                    .position(|effect| match effect_route(&effect.effect) {
                        EffectRoute::Telegram => !telegram_effect_active,
                        EffectRoute::LocalOrdered => !storage_effect_active,
                        EffectRoute::LocalIndependent => true,
                    });
            let Some(position) = position else {
                break;
            };
            let effect = pending_effects
                .remove(position)
                .expect("an effect position came from the same pending queue");
            match effect_route(&effect.effect) {
                EffectRoute::Telegram => telegram_effect_active = true,
                EffectRoute::LocalOrdered => storage_effect_active = true,
                EffectRoute::LocalIndependent => {}
            }
            if let Some(key) = notification_key(&effect.effect) {
                active_notifications.push(key);
            }
            active_effects.push(start_effect(backend.clone(), effect, peers.clone()));
        }

        if update.view.has_pending_effort() {
            if animation_timer.is_none() {
                animation_timer = Some(Box::pin(compio::time::sleep(Duration::from_millis(90))));
            }
        } else {
            animation_timer = None;
        }

        let wake = poll_fn(|cx| {
            wake_poller.poll(
                WakeSources {
                    ui: terminal,
                    events,
                    adapter_events,
                    active_effects: &mut active_effects,
                    backend: &backend,
                    animation_timer: &mut animation_timer,
                },
                WakePolicy {
                    poll_adapter: !disconnected,
                    poll_interaction: requested_exit.is_none(),
                    poll_background: !disconnected && requested_exit.is_none(),
                },
                cx,
            )
        })
        .await;

        match wake {
            ApplicationWake::Redraw(result) => result.context(TerminalSnafu)?,
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
            ApplicationWake::Adapter(event) => match *event {
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
                        pending_effects.clear();
                        requested_exit = Some(RequestedExit::Quit);
                        stopping_error = Some(error);
                        backend.begin_shutdown();
                        continue;
                    };
                    disconnected = true;
                    backend.begin_shutdown();
                    update = app.transition(Input::Adapter(AdapterEvent::ConnectionFailed(reason)));
                }
            },
            ApplicationWake::Backend(completion) => {
                match effect_route(&completion.effect.effect) {
                    EffectRoute::Telegram => telegram_effect_active = false,
                    EffectRoute::LocalOrdered => storage_effect_active = false,
                    EffectRoute::LocalIndependent => {}
                }
                if let Some(key) = notification_key(&completion.effect.effect) {
                    active_notifications.retain(|active| active != &key);
                }
                match completion.result {
                    Ok(output) => {
                        peers.merge(output.peers);
                        if let Some(returned) = output.telegram_update {
                            adapter_events.submit_update(returned);
                        }
                        if let Some(AdapterEvent::AccountLifecycleReady(request)) =
                            output.event.as_ref()
                        {
                            pending_effects.clear();
                            requested_exit = Some(RequestedExit::Lifecycle(*request));
                            continue;
                        }
                        if requested_exit.is_some() {
                            continue;
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
                        if disconnected && matches!(&error, Error::TelegramActorCancelled) {
                            pending_effects.push_front(completion.effect);
                            continue;
                        }
                        if requested_exit.is_some() {
                            if !matches!(&error, Error::TelegramActorCancelled)
                                && connection_failure_reason(&error).is_none()
                                && stopping_error.is_none()
                            {
                                stopping_error = Some(error);
                            }
                            continue;
                        }
                        let Some(reason) = connection_failure_reason(&error) else {
                            pending_effects.clear();
                            requested_exit = Some(RequestedExit::Quit);
                            stopping_error = Some(error);
                            backend.begin_shutdown();
                            continue;
                        };
                        pending_effects.push_front(completion.effect);
                        disconnected = true;
                        backend.begin_shutdown();
                        update =
                            app.transition(Input::Adapter(AdapterEvent::ConnectionFailed(reason)));
                    }
                }
            }
            ApplicationWake::Background(result) => match *result {
                Ok(output) => {
                    peers.merge(output.peers);
                    if let Some(returned) = output.telegram_update {
                        adapter_events.submit_update(returned);
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
                        pending_effects.clear();
                        requested_exit = Some(RequestedExit::Quit);
                        stopping_error = Some(error);
                        backend.begin_shutdown();
                        continue;
                    };
                    disconnected = true;
                    backend.begin_shutdown();
                    update = app.transition(Input::Adapter(AdapterEvent::ConnectionFailed(reason)));
                }
            },
            ApplicationWake::Animation => {
                animation_timer = None;
                update = app.transition(Input::Intent(Intent::Animate));
            }
        }
    }
}

#[derive(Clone, Copy)]
enum RequestedExit {
    Quit,
    Lifecycle(AccountLifecycle),
}
