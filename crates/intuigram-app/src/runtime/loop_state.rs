use super::types::{NotificationKey, PendingEffect};
use super::*;

#[cfg(test)]
pub(crate) async fn run_application<U, E, A, B>(
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
    let mut app = App::new();
    let update = app.transition(Input::Adapter(AdapterEvent::Bootstrap(bootstrap)));
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
            media_limits: intuigram_telegram::MediaLimits::default(),
        },
    )
    .await
}

pub(super) fn replace_update(current: &mut Update, next: Update, draw_requested: &mut bool) {
    *draw_requested |= current.view != next.view;
    *current = next;
}

pub(super) fn configure_small_media_capacity(app: &mut App, update: &mut Update, capacity: usize) {
    let configured = app.transition(Input::ConfigureSmallMediaCapacity(capacity));
    debug_assert!(
        configured.effect.is_none(),
        "media-capacity configuration does not request adapter work"
    );
    update.view = configured.view;
}

pub(super) fn draw_and_report_visible_avatars<U: ApplicationUi>(
    terminal: &mut U,
    app: &mut App,
    update: &mut Update,
    reported_avatar_peers: &mut Vec<ChatId>,
    draw_requested: &mut bool,
) -> Result<Option<Effect>> {
    terminal.draw(&update.view).context(TerminalSnafu)?;
    *draw_requested = false;
    let visible_avatar_peers = terminal.visible_avatar_peers(&update.view);
    if *reported_avatar_peers == visible_avatar_peers {
        return Ok(None);
    }
    *reported_avatar_peers = visible_avatar_peers.clone();
    let visible = app.transition(Input::SetVisibleAvatarPeers(visible_avatar_peers));
    *draw_requested |= update.view != visible.view;
    update.view = visible.view;
    Ok(visible.effect)
}

pub(super) fn enqueue_or_begin_shutdown<B: ApplicationBackend>(
    pending_effects: &mut VecDeque<AdapterEffect>,
    active_effects: &futures_util::stream::FuturesUnordered<PendingEffect>,
    active_notifications: &[NotificationKey],
    effect: Option<Effect>,
    requested_exit: &mut Option<RequestedExit>,
    backend: &B,
) -> Result<()> {
    if requested_exit.is_none()
        && enqueue_effect(
            pending_effects,
            active_effects,
            active_notifications,
            effect,
        )?
    {
        *requested_exit = Some(RequestedExit::Quit);
        pending_effects.clear();
        backend.begin_shutdown();
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_effect_admission<U, B>(
    terminal: &mut U,
    app: &mut App,
    update: &mut Update,
    reported_avatar_peers: &mut Vec<ChatId>,
    draw_requested: &mut bool,
    pending_effects: &mut VecDeque<AdapterEffect>,
    active_effects: &mut futures_util::stream::FuturesUnordered<PendingEffect>,
    active_notifications: &[NotificationKey],
    requested_exit: &mut Option<RequestedExit>,
    backend: &B,
) -> Result<()>
where
    U: ApplicationUi,
    B: ApplicationBackend,
{
    if *draw_requested {
        let visible_effect = draw_and_report_visible_avatars(
            terminal,
            app,
            update,
            reported_avatar_peers,
            draw_requested,
        )?;
        enqueue_or_begin_shutdown(
            pending_effects,
            active_effects,
            active_notifications,
            visible_effect,
            requested_exit,
            backend,
        )?;
    }
    cancel_superseded_work(active_effects, update.effect.as_ref());
    enqueue_or_begin_shutdown(
        pending_effects,
        active_effects,
        active_notifications,
        update.effect.take(),
        requested_exit,
        backend,
    )
}

pub(super) fn decrement_lane(active: &mut HashMap<Option<i32>, usize>, data_center: Option<i32>) {
    let Some(count) = active.get_mut(&data_center) else {
        return;
    };
    *count = count.saturating_sub(1);
    if *count == 0 {
        active.remove(&data_center);
    }
}

#[derive(Clone, Copy)]
pub(super) enum RequestedExit {
    Quit,
    Lifecycle(AccountLifecycle),
}
