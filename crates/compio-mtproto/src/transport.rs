use std::pin::Pin;
use std::task::{Context, Poll};

use futures_util::future::poll_fn;

use crate::TransportError;

/// Completion-friendly framed transport for serialized MTProto envelopes.
///
/// Implementations retain ownership of an outgoing buffer from `queue_send`
/// until `poll_send` completes. This keeps proxy and obfuscated transports on
/// Compio's owned-buffer path without requiring Tokio I/O compatibility.
pub trait Transport {
    /// Queues one serialized MTProto envelope for the next send operation.
    fn queue_send(self: Pin<&mut Self>, payload: Vec<u8>);

    /// Drives the queued send to completion.
    fn poll_send(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), TransportError>>;

    /// Polls for one complete serialized MTProto envelope.
    fn poll_receive(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Vec<u8>, TransportError>>;
}

/// Type-erased MTProto transport used by login and the persistent driver.
pub struct BoxedTransport {
    inner: Pin<Box<dyn Transport>>,
}

impl BoxedTransport {
    /// Erases one concrete transport while retaining its completion semantics.
    #[must_use]
    pub fn new<T>(transport: T) -> Self
    where
        T: Transport + 'static,
    {
        Self {
            inner: Box::pin(transport),
        }
    }

    /// Sends one already-serialized MTProto envelope.
    pub async fn send(&mut self, payload: Vec<u8>) -> Result<(), TransportError> {
        self.inner.as_mut().queue_send(payload);
        poll_fn(|cx| self.inner.as_mut().poll_send(cx)).await
    }

    /// Receives one complete serialized MTProto envelope.
    pub async fn receive(&mut self) -> Result<Vec<u8>, TransportError> {
        poll_fn(|cx| self.inner.as_mut().poll_receive(cx)).await
    }

    pub(crate) fn queue_send(&mut self, payload: Vec<u8>) {
        self.inner.as_mut().queue_send(payload);
    }

    pub(crate) fn poll_send(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), TransportError>> {
        self.inner.as_mut().poll_send(cx)
    }

    pub(crate) fn poll_receive(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Vec<u8>, TransportError>> {
        self.inner.as_mut().poll_receive(cx)
    }
}

impl Transport for BoxedTransport {
    fn queue_send(mut self: Pin<&mut Self>, payload: Vec<u8>) {
        self.inner.as_mut().queue_send(payload);
    }

    fn poll_send(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), TransportError>> {
        self.inner.as_mut().poll_send(cx)
    }

    fn poll_receive(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Vec<u8>, TransportError>> {
        self.inner.as_mut().poll_receive(cx)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::pin::Pin;
    use std::rc::Rc;
    use std::task::{Context, Poll};

    use super::{BoxedTransport, Transport};
    use crate::TransportError;

    struct ScriptedTransport {
        queued: Option<Vec<u8>>,
        sent: Rc<RefCell<Vec<Vec<u8>>>>,
        received: VecDeque<Vec<u8>>,
    }

    impl Transport for ScriptedTransport {
        fn queue_send(mut self: Pin<&mut Self>, payload: Vec<u8>) {
            self.queued = Some(payload);
        }

        fn poll_send(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), TransportError>> {
            let payload = self
                .queued
                .take()
                .expect("the boxed adapter polls only after queueing a payload");
            self.sent.borrow_mut().push(payload);
            Poll::Ready(Ok(()))
        }

        fn poll_receive(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<Vec<u8>, TransportError>> {
            Poll::Ready(
                self.received
                    .pop_front()
                    .ok_or(TransportError::ConnectionClosed),
            )
        }
    }

    #[test]
    fn boxed_transport_preserves_owned_buffers_for_proxy_adapters() {
        let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");
        runtime.block_on(async {
            let sent = Rc::new(RefCell::new(Vec::new()));
            let mut transport = BoxedTransport::new(ScriptedTransport {
                queued: None,
                sent: Rc::clone(&sent),
                received: VecDeque::from([vec![5, 6, 7, 8]]),
            });

            transport
                .send(vec![1, 2, 3, 4])
                .await
                .expect("scripted send should complete");
            assert_eq!(*sent.borrow(), vec![vec![1, 2, 3, 4]]);
            assert_eq!(
                transport
                    .receive()
                    .await
                    .expect("scripted receive should complete"),
                vec![5, 6, 7, 8]
            );
        });
    }
}
