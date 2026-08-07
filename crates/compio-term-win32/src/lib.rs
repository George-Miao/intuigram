//! Safe boundary around Compio's Win32 wait-handle operation.
//!
//! This small platform crate contains the unsafe contract required to attach a
//! console-input handle to Compio's IOCP driver. `compio-term` itself remains
//! free of platform FFI and receives only wake-driven readiness.

#![cfg(windows)]

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use compio::BufResult;
use compio::driver::{OpCode, OpType};
use compio::runtime::Submit;
use crossterm_winapi::Handle;
use windows_sys::Win32::System::IO::OVERLAPPED;

/// Persistent wait source for the current Win32 console input buffer.
pub struct ConsoleInput {
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
    /// Opens the current console input buffer.
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            handle: Handle::current_in_handle()?,
            wait: None,
        })
    }

    /// Polls Compio's IOCP driver for console-input readiness.
    pub fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
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
