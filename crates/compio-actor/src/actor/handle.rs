use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_channel::oneshot;
use snafu::Snafu;

/// The observed result of an actor task.
#[derive(Debug, PartialEq, Eq)]
pub enum ActorExit<E: Send + 'static> {
    /// The actor stopped normally.
    Stopped,
    /// A lifecycle method failed.
    Failed(E),
}

/// The worker stopped before reporting an actor's exit.
#[derive(Debug, PartialEq, Eq, Snafu)]
#[snafu(display("actor worker stopped before reporting an exit"))]
pub struct ActorHandleError;

/// A handle for observing an actor running in the cluster.
pub struct ActorHandle<E: Send + 'static> {
    pub(crate) result: oneshot::Receiver<Result<ActorExit<E>, ()>>,
}

impl<E: Send + 'static> Future for ActorHandle<E> {
    type Output = Result<ActorExit<E>, ActorHandleError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.get_mut().result)
            .poll(cx)
            .map(|result| result.ok().and_then(Result::ok).ok_or(ActorHandleError))
    }
}
