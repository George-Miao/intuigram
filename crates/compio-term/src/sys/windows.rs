use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use compio::BufResult;
use compio::driver::{OpCode, OpType};
use compio::runtime::Submit;
use crossterm::event::Event;
use crossterm_winapi::Handle;
use futures_util::stream::{FusedStream, Stream};
use windows_sys::Win32::System::IO::OVERLAPPED;

use crate::event::Result;

/// Persistent wait source for the current Win32 console input buffer.
struct ConsoleInput {
    handle: Handle,
    wait: Option<Submit<WaitConsole>>,
}

impl std::fmt::Debug for ConsoleInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ConsoleInput")
            .finish_non_exhaustive()
    }
}

impl ConsoleInput {
    fn new() -> io::Result<Self> {
        Ok(Self::from_handle(Handle::current_in_handle()?))
    }

    fn from_handle(handle: Handle) -> Self {
        Self { handle, wait: None }
    }

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let wait = self.wait.get_or_insert_with(|| {
            compio::runtime::submit(WaitConsole {
                handle: self.handle.clone(),
            })
        });
        match Pin::new(wait).poll(cx) {
            Poll::Ready(BufResult(result, _)) => {
                self.wait = None;
                Poll::Ready(result.map(|_| ()))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

#[derive(Debug)]
struct WaitConsole {
    handle: Handle,
}

// SAFETY: `handle` owns a stable reference-counted Win32 console handle for
// the complete operation. `operate` neither dereferences the IOCP pointer nor
// accesses thread-affine state; the driver only waits for this handle to be
// signalled and then reports readiness.
unsafe impl OpCode for WaitConsole {
    type Control = ();

    fn op_type(&self, _control: &Self::Control) -> OpType {
        OpType::Event(*self.handle as _)
    }

    unsafe fn operate(
        &mut self,
        _control: &mut Self::Control,
        _operation: *mut OVERLAPPED,
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(Ok(0))
    }
}

#[derive(Debug)]
pub(crate) struct EventStream {
    input: ConsoleInput,
    terminated: bool,
}

impl EventStream {
    pub(crate) fn new() -> Result<Self> {
        Ok(Self {
            input: ConsoleInput::new()?,
            terminated: false,
        })
    }

    fn ready_event() -> Result<Option<Event>> {
        if crossterm::event::poll(Duration::ZERO)? {
            return crossterm::event::read().map(Some);
        }
        Ok(None)
    }

    fn finish_with(&mut self, error: io::Error) -> Poll<Option<Result<Event>>> {
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
                    if let Err(error) = result {
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

#[cfg(test)]
mod tests {
    use std::future::poll_fn;
    use std::io;
    use std::ptr;

    use crossterm_winapi::Handle;
    use windows_sys::Win32::System::Threading::{CreateEventW, SetEvent};

    use super::ConsoleInput;

    #[test]
    fn console_input_signalled_handle_completes_wait() {
        // SAFETY: Null security attributes and name request an unnamed event
        // whose handle is validated immediately below.
        let event = unsafe { CreateEventW(ptr::null(), 0, 0, ptr::null()) };
        assert!(
            !event.is_null(),
            "test event should be created: {}",
            io::Error::last_os_error(),
        );

        // SAFETY: The event is an exclusively owned, thread-safe kernel handle.
        let handle = unsafe { Handle::from_raw(event.cast()) };
        let signal = handle.clone();
        let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");
        runtime.block_on(async {
            let mut input = ConsoleInput::from_handle(handle);

            // SAFETY: `signal` keeps the valid event handle alive for this call.
            let signalled = unsafe { SetEvent((*signal).cast()) };
            assert_ne!(
                signalled,
                0,
                "test event should be signalled: {}",
                io::Error::last_os_error(),
            );

            poll_fn(|cx| input.poll_ready(cx))
                .await
                .expect("signalled handle should wake the console input source");
        });
    }
}