use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use compio_term_win32::ConsoleInput;
use crossterm::event::Event;
use futures_util::stream::{FusedStream, Stream};
use snafu::ResultExt;

use crate::event::{
    OpenConsoleSnafu, PollConsoleSnafu, PollEventSnafu, ReadEventSnafu, Result,
};

#[derive(Debug)]
pub(crate) struct EventStream {
    input: ConsoleInput,
    terminated: bool,
}

impl EventStream {
    pub(crate) fn new() -> Result<Self> {
        Ok(Self {
            input: ConsoleInput::new().context(OpenConsoleSnafu)?,
            terminated: false,
        })
    }

    fn ready_event() -> Result<Option<Event>> {
        if crossterm::event::poll(Duration::ZERO).context(PollEventSnafu)? {
            return crossterm::event::read().map(Some).context(ReadEventSnafu);
        }
        Ok(None)
    }

    fn finish_with(&mut self, error: crate::event::Error) -> Poll<Option<Result<Event>>> {
        self.terminated = true;
        Poll::Ready(Some(Err(error)))
    }
}

impl Stream for EventStream {
    type Item = Result<Event>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.terminated {
            return Poll::Ready(None);
        }
        loop {
            match Self::ready_event() {
                Ok(Some(event)) => return Poll::Ready(Some(Ok(event))),
                Ok(None) => {}
                Err(error) => return self.finish_with(error),
            }
            match self.input.poll_ready(cx) {
                Poll::Ready(result) => {
                    if let Err(error) = result.context(PollConsoleSnafu) {
                        return self.finish_with(error);
                    }
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl FusedStream for EventStream {
    fn is_terminated(&self) -> bool {
        self.terminated
    }
}
