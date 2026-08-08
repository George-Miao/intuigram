use std::cell::RefCell;
use std::future::poll_fn;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Poll;

use futures_util::{Future, Stream};

use super::super::runtime_adapters::WorkerAdapterEvents;
use super::super::{AdapterBatch, ApplicationAdapterEvents, Error, Result};

pub(crate) struct ActorEvents {
    stream: Pin<Box<dyn Stream<Item = SessionEvent> + Send>>,
}

pub(super) enum SessionEvent {
    Batch(Box<AdapterBatch>),
    Failed(Box<Error>),
}

#[derive(Clone, Default)]
pub(super) struct DriverStop {
    state: Rc<RefCell<DriverStopState>>,
}

#[derive(Default)]
struct DriverStopState {
    stopped: bool,
    waker: Option<std::task::Waker>,
}

impl ActorEvents {
    pub(super) fn new(receiver: flume::Receiver<SessionEvent>) -> Self {
        Self {
            stream: Box::pin(receiver.into_stream()),
        }
    }
}

impl DriverStop {
    pub(super) fn stop(&self) {
        let mut state = self.state.borrow_mut();
        state.stopped = true;
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }

    fn poll(&self, cx: &mut std::task::Context<'_>) -> Poll<()> {
        let mut state = self.state.borrow_mut();
        if state.stopped {
            return Poll::Ready(());
        }
        if state
            .waker
            .as_ref()
            .is_none_or(|waker| !waker.will_wake(cx.waker()))
        {
            state.waker = Some(cx.waker().clone());
        }
        Poll::Pending
    }
}

impl ApplicationAdapterEvents for ActorEvents {
    fn poll_adapter_event(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<AdapterBatch>> {
        match self.stream.as_mut().poll_next(cx) {
            Poll::Ready(Some(SessionEvent::Batch(batch))) => Poll::Ready(Ok(*batch)),
            Poll::Ready(Some(SessionEvent::Failed(error))) => Poll::Ready(Err(*error)),
            Poll::Ready(None) => Poll::Ready(Err(Error::TelegramUpdatesClosed)),
            Poll::Pending => Poll::Pending,
        }
    }
}

pub(super) async fn run_driver<A>(
    mut events: A,
    stop: DriverStop,
    output: flume::Sender<SessionEvent>,
) where
    A: WorkerAdapterEvents,
{
    loop {
        let next = poll_fn(|cx| {
            if stop.poll(cx).is_ready() {
                return Poll::Ready(None);
            }
            events.poll_worker_event(cx).map(Some)
        })
        .await;
        let Some(result) = next else {
            events.close();
            return;
        };
        let (event, delivered) = match result {
            Ok(batch) => {
                let delivered = batch.delivered;
                (SessionEvent::Batch(Box::new(batch.batch)), delivered)
            }
            Err(error) => (SessionEvent::Failed(Box::new(error)), None),
        };
        let failed = matches!(event, SessionEvent::Failed(_));
        let sent = send_or_stop(&output, &stop, event).await;
        if let Some(delivered) = delivered {
            if sent {
                delivered.complete(Ok(()));
            }
        }
        if !sent || failed {
            events.close();
            return;
        }
    }
}

async fn send_or_stop(
    output: &flume::Sender<SessionEvent>,
    stop: &DriverStop,
    event: SessionEvent,
) -> bool {
    let mut send = std::pin::pin!(output.send_async(event));
    poll_fn(|cx| {
        if stop.poll(cx).is_ready() {
            return Poll::Ready(false);
        }
        send.as_mut().poll(cx).map(|result| result.is_ok())
    })
    .await
}
