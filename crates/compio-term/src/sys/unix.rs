use std::fs::{File, OpenOptions};
use std::io::{IsTerminal, Read};
use std::os::unix::net::UnixStream;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use compio::runtime::fd::PollFd;
use crossterm::event::Event;
use futures_util::stream::{FusedStream, Stream};
use signal_hook::SigId;
use signal_hook::consts::SIGWINCH;
use signal_hook::low_level::pipe;
use crate::event::Result;

#[derive(Debug)]
pub(crate) struct EventStream {
    tty: PollFd<File>,
    resize: ResizeWake,
    terminated: bool,
}

impl EventStream {
    pub(crate) fn new() -> Result<Self> {
        let path = if std::io::stdin().is_terminal() {
            "/dev/stdin"
        } else {
            "/dev/tty"
        };
        let tty = OpenOptions::new()
            .read(true)
            .open(path)?;
        let tty = PollFd::new(tty)?;
        Ok(Self {
            tty,
            resize: ResizeWake::new()?,
            terminated: false,
        })
    }

    fn ready_event() -> Result<Option<Event>> {
        if crossterm::event::poll(Duration::ZERO)? {
            return crossterm::event::read().map(Some);
        }
        Ok(None)
    }

    fn finish_with(&mut self, error: std::io::Error) -> Poll<Option<Result<Event>>> {
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

            let tty_ready = self.tty.poll_read_ready(cx);
            let resize_ready = self.resize.poll_ready(cx);

            match tty_ready {
                Poll::Ready(result) => {
                    if let Err(error) = result {
                        return self.finish_with(error);
                    }
                    continue;
                }
                Poll::Pending => {}
            }

            match resize_ready {
                Poll::Ready(result) => {
                    if let Err(error) = result {
                        return self.finish_with(error);
                    }
                    continue;
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

#[derive(Debug)]
struct ResizeWake {
    source: PollFd<UnixStream>,
    registration: SigId,
}

impl ResizeWake {
    fn new() -> Result<Self> {
        Self::for_signal(SIGWINCH)
    }

    fn for_signal(signal: i32) -> Result<Self> {
        let (reader, writer) = UnixStream::pair()?;
        reader.set_nonblocking(true)?;
        let registration = pipe::register(signal, writer)?;
        let source = PollFd::new(reader)?;
        Ok(Self {
            source,
            registration,
        })
    }

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<()>> {
        match self.source.poll_read_ready(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(result) => {
                result?;
                self.drain()?;
                Poll::Ready(Ok(()))
            }
        }
    }

    fn drain(&self) -> Result<()> {
        let mut source = &*self.source;
        let mut buffer = [0_u8; 64];
        loop {
            match source.read(&mut buffer) {
                Ok(0) => return Ok(()),
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) => return Err(error),
            }
        }
    }
}

impl Drop for ResizeWake {
    fn drop(&mut self) {
        signal_hook::low_level::unregister(self.registration);
    }
}

#[cfg(test)]
mod tests {
    use std::future::poll_fn;

    use signal_hook::consts::SIGUSR1;

    use super::ResizeWake;

    #[test]
    fn signal_wakes_the_compio_resize_source() {
        let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");
        runtime.block_on(async {
            let mut wake =
                ResizeWake::for_signal(SIGUSR1).expect("test signal wake source should initialize");
            signal_hook::low_level::raise(SIGUSR1).expect("test signal should be raised");
            poll_fn(|cx| wake.poll_ready(cx))
                .await
                .expect("signal readiness should be observed");
        });
    }
}
