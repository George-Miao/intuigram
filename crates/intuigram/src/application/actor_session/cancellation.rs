use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::Poll;

use futures_util::Future;
use futures_util::task::AtomicWaker;

use super::super::{Error, Result};

#[derive(Clone, Default)]
pub(super) struct ActorCancellation {
    inner: Arc<CancellationState>,
}

#[derive(Default)]
struct CancellationState {
    cancelled: AtomicBool,
    waker: AtomicWaker,
}

impl ActorCancellation {
    pub(super) fn cancel(&self) {
        self.inner.cancelled.store(true, Ordering::Release);
        self.inner.waker.wake();
    }

    pub(super) fn poll(&self, cx: &mut std::task::Context<'_>) -> Poll<()> {
        if self.inner.cancelled.load(Ordering::Acquire) {
            return Poll::Ready(());
        }
        self.inner.waker.register(cx.waker());
        if self.inner.cancelled.load(Ordering::Acquire) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

pub(super) async fn until_cancelled<T>(
    operation: impl Future<Output = Result<T>>,
    cancellation: &ActorCancellation,
) -> Result<T> {
    let mut operation = std::pin::pin!(operation);
    std::future::poll_fn(|cx| {
        if let Poll::Ready(result) = operation.as_mut().poll(cx) {
            return Poll::Ready(result);
        }
        if cancellation.poll(cx).is_ready() {
            return Poll::Ready(Err(Error::TelegramActorCancelled));
        }
        Poll::Pending
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_operation_wins_over_simultaneous_cancellation() {
        let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");
        runtime.block_on(async {
            let cancellation = ActorCancellation::default();
            cancellation.cancel();

            assert_eq!(
                until_cancelled(async { Ok(7) }, &cancellation)
                    .await
                    .expect("a completed operation must not be discarded"),
                7
            );
        });
    }
}
