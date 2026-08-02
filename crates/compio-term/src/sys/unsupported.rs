use std::pin::Pin;
use std::task::{Context, Poll};

use crossterm::event::Event;
use futures_util::stream::{FusedStream, Stream};

use crate::event::{Result, UnsupportedPlatformSnafu};

#[derive(Debug)]
pub(crate) struct EventStream;

impl EventStream {
    pub(crate) fn new() -> Result<Self> {
        UnsupportedPlatformSnafu.fail()
    }
}

impl Stream for EventStream {
    type Item = Result<Event>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(None)
    }
}

impl FusedStream for EventStream {
    fn is_terminated(&self) -> bool {
        true
    }
}
