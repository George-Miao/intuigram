use std::task::Context;

use crossterm::event::Event;
use futures_util::task::noop_waker_ref;

use super::*;

const TEXT_INPUT_BATCH_LIMIT: usize = 64;

pub(crate) fn append_ready_text<U, E>(
    terminal: &U,
    events: &mut E,
    view: &intuigram_lib::View,
    text: &mut String,
) -> Result<Option<Event>>
where
    U: ApplicationUi,
    E: ApplicationEvents,
{
    let mut cx = Context::from_waker(noop_waker_ref());
    for _ in 1..TEXT_INPUT_BATCH_LIMIT {
        let Poll::Ready(event) = events.poll_next_event(&mut cx) else {
            return Ok(None);
        };
        let event = event.context(TerminalSnafu)?;
        match terminal.resolve_event(view, event.clone()) {
            Some(UiEvent::Intent(Intent::Insert(next))) => text.push_str(&next),
            _ => return Ok(Some(event)),
        }
    }
    Ok(None)
}
