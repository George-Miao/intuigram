use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use crossterm::event::Event;
use futures_util::stream::{FusedStream, Stream};

use crate::sys;

/// Result returned by terminal event operations.
pub type Result<T> = io::Result<T>;

/// A wake-driven stream of Crossterm events.
///
/// The stream must be constructed and polled on one Compio runtime thread.
/// Only one terminal event reader may be active in a process, and callers must
/// not mix it with Crossterm's `read`, `poll`, or `EventStream` APIs.
#[derive(Debug)]
pub struct EventStream(sys::EventStream);

impl EventStream {
    /// Opens the platform terminal event source.
    pub fn new() -> Result<Self> {
        sys::EventStream::new().map(Self)
    }
}

impl Stream for EventStream {
    type Item = Result<Event>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.get_mut().0).poll_next(cx)
    }
}

impl FusedStream for EventStream {
    fn is_terminated(&self) -> bool {
        self.0.is_terminated()
    }
}
