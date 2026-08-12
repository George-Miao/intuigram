use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

use futures_util::Stream;
use grammers_tl_types::{Deserializable, RemoteCall, Serializable};
use snafu::ResultExt;

use super::{Response, Shared};
use crate::sender::{DeserializeResponseSnafu, Result};

/// Cloneable raw invocation endpoint for one MTProto connection driver.
#[derive(Clone)]
pub struct InvocationHandle {
    pub(super) shared: Rc<Shared>,
}

impl InvocationHandle {
    /// Enqueues one serialized TL request without waiting for network progress.
    pub fn invoke_raw(&self, body: Vec<u8>) -> Result<Invocation> {
        self.shared.enqueue(body)
    }

    /// Invokes one typed Telegram method through the connection driver.
    pub async fn invoke<R>(&self, request: &R) -> Result<R::Return>
    where
        R: RemoteCall + Serializable,
        R::Return: Deserializable,
    {
        let body = self.invoke_raw(request.to_bytes())?.await?;
        R::Return::from_bytes(&body).context(DeserializeResponseSnafu)
    }

    /// Requests orderly driver shutdown and completes outstanding invocations.
    pub fn stop(&self) {
        self.shared.stop();
        if let Some(waker) = self.shared.driver_waker.borrow_mut().take() {
            waker.wake();
        }
    }
}

/// Awaitable completion of one raw MTProto invocation.
pub struct Invocation {
    pub(super) response: Rc<Response>,
    pub(super) shared: Rc<Shared>,
}

impl fmt::Debug for Invocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Invocation").finish_non_exhaustive()
    }
}

impl Future for Invocation {
    type Output = Result<Vec<u8>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(result) = self.response.result.borrow_mut().take() {
            return Poll::Ready(result);
        }
        *self.response.waker.borrow_mut() = Some(cx.waker().clone());
        Poll::Pending
    }
}

impl Drop for Invocation {
    fn drop(&mut self) {
        if self.response.completed.replace(true) {
            return;
        }
        self.shared
            .outstanding
            .set(self.shared.outstanding.get().saturating_sub(1));
        self.response.waker.borrow_mut().take();
    }
}

/// Passive raw Telegram updates produced independently of RPC activity.
pub struct UpdateStream {
    pub(super) shared: Rc<Shared>,
}

impl Stream for UpdateStream {
    type Item = Vec<u8>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(update) = self.shared.updates.borrow_mut().pop_front() {
            return Poll::Ready(Some(update));
        }
        if self.shared.stopped.get() {
            return Poll::Ready(None);
        }
        *self.shared.update_waker.borrow_mut() = Some(cx.waker().clone());
        Poll::Pending
    }
}

impl UpdateStream {
    /// Returns the number of updates already buffered for delivery.
    pub fn buffered_len(&self) -> usize {
        self.shared.updates.borrow().len()
    }
}
