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

/// One raw Telegram update and the request that produced it, when applicable.
#[derive(Clone, Eq, PartialEq)]
pub struct RawUpdate {
    body: Vec<u8>,
    request: Option<Vec<u8>>,
}

impl RawUpdate {
    pub(super) fn passive(body: Vec<u8>) -> Self {
        Self {
            body,
            request: None,
        }
    }

    pub(super) fn correlated(body: Vec<u8>, request: Option<Vec<u8>>) -> Self {
        Self { body, request }
    }

    /// Returns the serialized update body.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Returns the serialized RPC request that produced this update.
    ///
    /// The request can contain private user data. Do not log it.
    #[must_use]
    pub fn request(&self) -> Option<&[u8]> {
        self.request.as_deref()
    }
}

impl fmt::Debug for RawUpdate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RawUpdate")
            .field("body_bytes", &self.body.len())
            .field("has_request", &self.request.is_some())
            .finish()
    }
}

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
    type Item = RawUpdate;

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
