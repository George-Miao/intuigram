use std::pin::Pin;
use std::task::{Context, Poll};

use crossterm::event::Event;
use futures_util::stream::{FusedStream, Stream};
use snafu::Snafu;

use crate::sys;

/// Failure while opening or reading the terminal event source.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    /// The process has no accessible controlling terminal.
    #[snafu(display("failed to open the controlling terminal"))]
    OpenTty { source: std::io::Error },

    /// Compio could not construct a readiness source for the terminal.
    #[snafu(display("failed to create the Compio terminal readiness source"))]
    CreatePollSource { source: std::io::Error },

    /// The Unix resize wake pipe could not be created.
    #[snafu(display("failed to create the terminal resize wake source"))]
    CreateResizeWake { source: std::io::Error },

    /// The Unix resize wake pipe could not be configured for nonblocking reads.
    #[snafu(display("failed to configure the terminal resize wake source"))]
    ConfigureResizeWake { source: std::io::Error },

    /// The terminal resize signal could not be connected to its wake pipe.
    #[snafu(display("failed to register terminal resize notifications"))]
    RegisterResizeWake { source: std::io::Error },

    /// Compio failed while waiting for terminal input readiness.
    #[snafu(display("failed to await terminal input readiness"))]
    PollTty { source: std::io::Error },

    /// Compio failed while waiting for a terminal resize notification.
    #[snafu(display("failed to await terminal resize notification"))]
    PollResizeWake { source: std::io::Error },

    /// The terminal resize wake pipe could not be drained.
    #[snafu(display("failed to drain terminal resize notifications"))]
    DrainResizeWake { source: std::io::Error },

    /// Crossterm failed while checking its decoded-event buffer.
    #[snafu(display("failed to poll terminal events"))]
    PollEvent { source: std::io::Error },

    /// Crossterm failed while decoding a ready terminal event.
    #[snafu(display("failed to read a terminal event"))]
    ReadEvent { source: std::io::Error },

    /// The current platform needs a terminal-specific event backend.
    #[snafu(display("compio-term event input is not implemented on this platform"))]
    UnsupportedPlatform,
}

/// Result returned by terminal event operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

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
