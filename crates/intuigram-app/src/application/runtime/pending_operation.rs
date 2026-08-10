use super::super::*;

pub(in crate::application) async fn wait_for_reconnect_cleanup<U, E, F, T>(
    terminal: &mut U,
    events: &mut E,
    app: &mut App,
    update: &mut Update,
    pending_effects: &mut VecDeque<AdapterEffect>,
    cleanup: F,
) -> Result<Loading<T>>
where
    U: ApplicationUi,
    E: ApplicationEvents,
    F: Future<Output = Result<T>>,
{
    let mut cleanup = Box::pin(cleanup);
    let active = futures_util::stream::FuturesUnordered::new();
    let mut animation = Box::pin(compio::time::sleep(Duration::from_millis(90)));
    let mut exit = None;

    loop {
        terminal.draw(&update.view).context(TerminalSnafu)?;
        if let Some(effect) = update.effect.take() {
            match effect {
                Effect::Quit => exit = Some(AccountSessionExit::Quit),
                Effect::AccountLifecycle { request } => {
                    exit = Some(AccountSessionExit::Lifecycle(request));
                }
                Effect::Reconnect => {}
                effect => {
                    enqueue_effect(pending_effects, &active, &[], Some(effect))?;
                }
            }
        }

        enum Wake<T, C> {
            Terminal(T),
            Redraw(intuigram_tui::Result<()>),
            Cleaned(C),
            Animate,
        }

        let wake = poll_fn(|cx| {
            if let Poll::Ready(result) = terminal.poll_redraw(cx) {
                return Poll::Ready(Wake::Redraw(result));
            }
            if exit.is_none()
                && let Poll::Ready(event) = events.poll_next_event(cx)
            {
                return Poll::Ready(Wake::Terminal(event));
            }
            if let Poll::Ready(cleaned) = cleanup.as_mut().poll(cx) {
                return Poll::Ready(Wake::Cleaned(cleaned));
            }
            if animation.as_mut().poll(cx).is_ready() {
                return Poll::Ready(Wake::Animate);
            }
            Poll::Pending
        })
        .await;

        match wake {
            Wake::Redraw(result) => result.context(TerminalSnafu)?,
            Wake::Terminal(event) => {
                let event = event.context(TerminalSnafu)?;
                if let Some(UiEvent::Intent(intent)) = terminal.resolve_event(&update.view, event) {
                    *update = app.transition(Input::Intent(intent));
                }
            }
            Wake::Cleaned(cleaned) => {
                let cleaned = cleaned?;
                return Ok(match exit {
                    Some(outcome) => Loading::Exit(outcome),
                    None => Loading::Ready(cleaned),
                });
            }
            Wake::Animate => {
                animation = Box::pin(compio::time::sleep(Duration::from_millis(90)));
                *update = app.transition(Input::Intent(Intent::Animate));
            }
        }
    }
}
