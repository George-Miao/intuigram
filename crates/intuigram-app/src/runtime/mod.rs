use super::*;

mod adapters;
mod cancellation;
mod effect_route;
mod input;
mod loop_state;
mod pending_operation;
mod types;
mod update_ordering;
mod wake_poll;

#[cfg(test)]
pub(super) use adapters::WorkerBatch;
pub(super) use adapters::{
    AdapterBatch, ApplicationAdapterEvents, ApplicationBackend, ApplicationEvents, BackendOutput,
    WorkerAdapterEvents,
};
pub(super) use cancellation::EffectCancellation;
use cancellation::{cancel_superseded_work, cancelled_media_event};
pub(super) use effect_route::{EffectRoute, effect_data_center, effect_priority, effect_route};
pub(super) use input::append_ready_text;
#[cfg(test)]
pub(super) use loop_state::run_application;
use loop_state::{
    RequestedExit, configure_small_media_capacity, decrement_lane, prepare_effect_admission,
    replace_update,
};
pub(super) use pending_operation::wait_for_reconnect_cleanup;
#[cfg(test)]
pub(super) use types::PendingEffect;
pub(super) use types::{
    AccountSessionExit, AdapterEffect, ApplicationExit, ApplicationState, ApplicationWake,
    DisconnectedApplication, connection_failure_reason, enqueue_effect, notification_key,
    start_effect,
};
use wake_poll::{WakePolicy, WakePoller, WakeSources};

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
        media_limits,
    } = state;
    configure_small_media_capacity(&mut app, &mut update, media_limits.small);
    let mut active_effects = futures_util::stream::FuturesUnordered::new();
    let mut animation_timer: Option<Pin<Box<dyn Future<Output = ()>>>> = None;
    let mut wake_poller = WakePoller::new();
    let mut disconnected = false;
    let mut telegram_control_active = false;
    let mut small_media_active = HashMap::<Option<i32>, usize>::new();
    let mut large_transfer_active = HashMap::<Option<i32>, usize>::new();
    let mut storage_effect_active = false;
    let mut active_notifications = Vec::new();
    let mut requested_exit = None;
    let mut stopping_error = None;
    let mut pending_terminal_event: Option<crossterm::event::Event> = None;
    let mut draw_requested = true;
    let mut reported_avatar_peers = Vec::new();

    loop {
        prepare_effect_admission(
            terminal,
            &mut app,
            &mut update,
            &mut reported_avatar_peers,
            &mut draw_requested,
            &mut pending_effects,
            &mut active_effects,
            &active_notifications,
            &mut requested_exit,
            &backend,
        )?;

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
            let position = pending_effects
                .iter()
                .enumerate()
                .filter(|(_, effect)| match effect_route(&effect.effect) {
                    EffectRoute::TelegramControl => !telegram_control_active,
                    EffectRoute::SmallMedia => {
                        small_media_active
                            .get(&effect_data_center(&effect.effect))
                            .copied()
                            .unwrap_or_default()
                            < media_limits.small
                    }
                    EffectRoute::LargeTransfer => {
                        large_transfer_active
                            .get(&effect_data_center(&effect.effect))
                            .copied()
                            .unwrap_or_default()
                            < media_limits.large
                    }
                    EffectRoute::LocalOrdered => !storage_effect_active,
                    EffectRoute::LocalIndependent => true,
                })
                .min_by_key(|(_, effect)| effect_priority(&effect.effect))
                .map(|(position, _)| position);
            let Some(position) = position else {
                break;
            };
            let effect = pending_effects
                .remove(position)
                .expect("an effect position came from the same pending queue");
            let route = effect_route(&effect.effect);
            let admission = effect.effect.admission();
            match route {
                EffectRoute::TelegramControl => telegram_control_active = true,
                EffectRoute::SmallMedia => {
                    *small_media_active
                        .entry(effect_data_center(&effect.effect))
                        .or_default() += 1;
                }
                EffectRoute::LargeTransfer => {
                    *large_transfer_active
                        .entry(effect_data_center(&effect.effect))
                        .or_default() += 1;
                }
                EffectRoute::LocalOrdered => storage_effect_active = true,
                EffectRoute::LocalIndependent => {}
            }
            if let Some(key) = notification_key(&effect.effect) {
                active_notifications.push(key);
            }
            active_effects.push(start_effect(backend.clone(), effect, peers.clone()));
            if let Some(admission) = admission {
                let accepted = app.transition(Input::EffectAccepted(admission));
                draw_requested |= update.view != accepted.view;
                update.view = accepted.view;
                enqueue_effect(
                    &mut pending_effects,
                    &active_effects,
                    &active_notifications,
                    accepted.effect,
                )?;
            }
        }

        if draw_requested {
            continue;
        }

        if update.view.has_pending_effort() {
            if animation_timer.is_none() {
                animation_timer = Some(Box::pin(compio::time::sleep(Duration::from_millis(90))));
            }
        } else {
            animation_timer = None;
        }

        let wake = if let Some(event) = pending_terminal_event.take() {
            ApplicationWake::Terminal(Ok(event))
        } else {
            poll_fn(|cx| {
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
            .await
        };

        match wake {
            ApplicationWake::Redraw(result) => {
                result.context(TerminalSnafu)?;
                draw_requested = true;
            }
            ApplicationWake::Terminal(event) => {
                let event = event.context(TerminalSnafu)?;
                let Some(event) = terminal.resolve_event(&update.view, event) else {
                    continue;
                };
                let event = if let UiEvent::Intent(Intent::Insert(mut text)) = event {
                    pending_terminal_event =
                        append_ready_text(terminal, events, &update.view, &mut text)?;
                    UiEvent::Intent(Intent::Insert(text))
                } else {
                    event
                };
                match event {
                    UiEvent::Redraw => draw_requested = true,
                    UiEvent::Intent(intent) => {
                        let next = app.transition(Input::Intent(intent));
                        replace_update(&mut update, next, &mut draw_requested);
                    }
                }
            }
            ApplicationWake::Adapter(event) => match *event {
                Ok(batch) => {
                    peers.merge(batch.peers);
                    let next = match batch.event {
                        Some(event) => app.transition(Input::Adapter(event)),
                        None => Update {
                            view: app.view(),
                            effect: None,
                        },
                    };
                    replace_update(&mut update, next, &mut draw_requested);
                }
                Err(error) => {
                    let Some(reason) = connection_failure_reason(&error) else {
                        pending_effects.clear();
                        requested_exit = Some(RequestedExit::Quit);
                        stopping_error = Some(error);
                        backend.begin_shutdown();
                        continue;
                    };
                    logging::connection_interrupted(&reason);
                    disconnected = true;
                    backend.begin_shutdown();
                    let next =
                        app.transition(Input::Adapter(AdapterEvent::ConnectionFailed(reason)));
                    replace_update(&mut update, next, &mut draw_requested);
                }
            },
            ApplicationWake::Backend(completion) => {
                match effect_route(&completion.effect.effect) {
                    EffectRoute::TelegramControl => telegram_control_active = false,
                    EffectRoute::SmallMedia => decrement_lane(
                        &mut small_media_active,
                        effect_data_center(&completion.effect.effect),
                    ),
                    EffectRoute::LargeTransfer => {
                        decrement_lane(
                            &mut large_transfer_active,
                            effect_data_center(&completion.effect.effect),
                        );
                    }
                    EffectRoute::LocalOrdered => storage_effect_active = false,
                    EffectRoute::LocalIndependent => {}
                }
                if let Some(key) = notification_key(&completion.effect.effect) {
                    active_notifications.retain(|active| active != &key);
                }
                if completion.cancelled {
                    if let Some(event) = cancelled_media_event(&completion.effect.effect) {
                        let next = app.transition(Input::Adapter(event));
                        replace_update(&mut update, next, &mut draw_requested);
                    }
                    continue;
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
                        let next = match output.event {
                            Some(event) => app.transition(Input::Adapter(event)),
                            None => Update {
                                view: app.view(),
                                effect: None,
                            },
                        };
                        replace_update(&mut update, next, &mut draw_requested);
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
                        logging::connection_interrupted(&reason);
                        pending_effects.push_front(completion.effect);
                        disconnected = true;
                        backend.begin_shutdown();
                        let next =
                            app.transition(Input::Adapter(AdapterEvent::ConnectionFailed(reason)));
                        replace_update(&mut update, next, &mut draw_requested);
                    }
                }
            }
            ApplicationWake::Background(result) => match *result {
                Ok(output) => {
                    peers.merge(output.peers);
                    if let Some(returned) = output.telegram_update {
                        adapter_events.submit_update(returned);
                    }
                    let next = match output.event {
                        Some(event) => app.transition(Input::Adapter(event)),
                        None => Update {
                            view: app.view(),
                            effect: None,
                        },
                    };
                    replace_update(&mut update, next, &mut draw_requested);
                }
                Err(error) => {
                    let Some(reason) = connection_failure_reason(&error) else {
                        pending_effects.clear();
                        requested_exit = Some(RequestedExit::Quit);
                        stopping_error = Some(error);
                        backend.begin_shutdown();
                        continue;
                    };
                    logging::connection_interrupted(&reason);
                    disconnected = true;
                    backend.begin_shutdown();
                    let next =
                        app.transition(Input::Adapter(AdapterEvent::ConnectionFailed(reason)));
                    replace_update(&mut update, next, &mut draw_requested);
                }
            },
            ApplicationWake::Animation => {
                animation_timer = None;
                let next = app.transition(Input::Intent(Intent::Animate));
                replace_update(&mut update, next, &mut draw_requested);
            }
        }
    }
}
