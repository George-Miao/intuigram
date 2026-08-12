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
