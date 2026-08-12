use std::cell::Cell;
use std::num::NonZeroUsize;
use std::pin::{Pin, pin};
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use futures_util::{FutureExt as _, StreamExt};
use grammers_mtproto::mtp::{BadMessage, Deserialization, RpcResult};

use super::{ConnectionDriver, InvocationHandle, Shared, UpdateStream};
use crate::{AuthKeyMaterial, BoxedTransport, Transport, TransportError};

#[test]
fn telegram_flood_wait_exposes_its_retry_delay() {
    let error = crate::InvocationError::Rpc {
        code: 420,
        message: "FLOOD_WAIT_17".to_owned(),
    };

    assert_eq!(error.retry_after(), Some(Duration::from_secs(17)));
}

struct PendingTransport {
    queued: Option<Vec<u8>>,
}

impl Transport for PendingTransport {
    fn queue_send(mut self: Pin<&mut Self>, payload: Vec<u8>) {
        self.queued = Some(payload);
    }

    fn poll_send(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), TransportError>> {
        self.queued.take();
        Poll::Ready(Ok(()))
    }

    fn poll_receive(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Vec<u8>, TransportError>> {
        Poll::Pending
    }
}

struct RecordingTransport {
    queued: Option<Vec<u8>>,
    sends: Rc<Cell<usize>>,
}

impl Transport for RecordingTransport {
    fn queue_send(mut self: Pin<&mut Self>, payload: Vec<u8>) {
        self.queued = Some(payload);
    }

    fn poll_send(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), TransportError>> {
        self.queued
            .take()
            .expect("the driver polls sending only after queueing a payload");
        self.sends.set(self.sends.get() + 1);
        Poll::Ready(Ok(()))
    }

    fn poll_receive(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Vec<u8>, TransportError>> {
        Poll::Pending
    }
}

fn test_parts(capacity: NonZeroUsize) -> (InvocationHandle, UpdateStream, Rc<Shared>) {
    let shared = Shared::new(capacity);
    (
        InvocationHandle {
            shared: Rc::clone(&shared),
        },
        UpdateStream {
            shared: Rc::clone(&shared),
        },
        shared,
    )
}

fn driver_parts() -> (InvocationHandle, UpdateStream, ConnectionDriver) {
    let material = AuthKeyMaterial {
        auth_key: [0x42; 256],
        time_offset: 0,
        first_salt: 0,
    };
    let connection = super::super::EncryptedConnection::from_boxed(
        BoxedTransport::new(PendingTransport { queued: None }),
        &material,
    );
    connection.into_driver(NonZeroUsize::new(8).expect("fixture capacity is positive"))
}

fn poll_driver(driver: &mut Pin<Box<ConnectionDriver>>) {
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");
    runtime.block_on(std::future::poll_fn(|cx| {
        assert!(driver.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    }));
}

#[test]
fn invocation_queue_is_bounded_before_network_progress() {
    let (handle, _updates, _shared) =
        test_parts(NonZeroUsize::new(1).expect("fixture capacity is positive"));

    let _first = handle
        .invoke_raw(vec![1, 2, 3, 4])
        .expect("first invocation should fit");
    let error = handle
        .invoke_raw(vec![5, 6, 7, 8])
        .expect_err("second invocation should exceed capacity");

    assert!(matches!(error, super::Error::QueueFull { capacity: 1 }));
}

#[test]
fn dropping_an_invocation_immediately_releases_its_capacity() {
    let (handle, _updates, shared) =
        test_parts(NonZeroUsize::new(1).expect("fixture capacity is positive"));
    let first = handle
        .invoke_raw(vec![1, 2, 3, 4])
        .expect("first invocation should fit");

    drop(first);
    let _second = handle
        .invoke_raw(vec![5, 6, 7, 8])
        .expect("cancelled invocation should release capacity");

    assert_eq!(shared.outstanding.get(), 1);
}

#[test]
fn cancelling_an_in_flight_request_does_not_block_a_new_request() {
    let material = AuthKeyMaterial {
        auth_key: [0x42; 256],
        time_offset: 0,
        first_salt: 0,
    };
    let connection = super::super::EncryptedConnection::from_boxed(
        BoxedTransport::new(PendingTransport { queued: None }),
        &material,
    );
    let (handle, _updates, driver) =
        connection.into_driver(NonZeroUsize::new(1).expect("fixture capacity is positive"));
    let first = handle
        .invoke_raw(vec![1, 2, 3, 4])
        .expect("first invocation should fit");
    let mut driver = Box::pin(driver);
    poll_driver(&mut driver);

    drop(first);
    let _second = handle
        .invoke_raw(vec![5, 6, 7, 8])
        .expect("cancelled in-flight request should release capacity");
    poll_driver(&mut driver);

    assert_eq!(driver.in_flight.len(), 2);
}

#[test]
fn passive_updates_are_awaitable_without_an_rpc() {
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");
    runtime.block_on(async {
        let (_handle, mut updates, shared) =
            test_parts(NonZeroUsize::new(1).expect("fixture capacity is positive"));
        shared.publish_update(vec![0x42, 0x24]);

        assert_eq!(updates.next().await, Some(vec![0x42, 0x24]));
    });
}

#[test]
fn idle_driver_sends_keepalive_without_capacity() {
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");
    runtime.block_on(async {
        let material = AuthKeyMaterial {
            auth_key: [0x42; 256],
            time_offset: 0,
            first_salt: 0,
        };
        let sends = Rc::new(Cell::new(0));
        let connection = super::super::EncryptedConnection::from_boxed(
            BoxedTransport::new(RecordingTransport {
                queued: None,
                sends: Rc::clone(&sends),
            }),
            &material,
        );
        let (handle, _updates, mut driver) =
            connection.into_driver(NonZeroUsize::new(1).expect("fixture capacity is positive"));
        driver.keepalive = super::keepalive::Keepalive::new(Duration::ZERO);
        let mut driver = Box::pin(driver);

        std::future::poll_fn(|cx| {
            assert!(driver.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;

        assert_eq!(sends.get(), 1);
        let _request = handle
            .invoke_raw(vec![1, 2, 3, 4])
            .expect("keepalive traffic must not consume invocation capacity");
    });
}

#[test]
fn rpc_results_complete_only_the_correlated_invocation() {
    let (handle, _updates, driver) = driver_parts();
    let mut first = pin!(handle.invoke_raw(vec![1, 2, 3, 4]).expect("request fits"));
    let mut second = pin!(handle.invoke_raw(vec![5, 6, 7, 8]).expect("request fits"));
    let mut driver = Box::pin(driver);
    poll_driver(&mut driver);
    assert_eq!(driver.in_flight.len(), 2);
    let second_id = driver
        .in_flight
        .iter()
        .find_map(|(id, request)| (request.body == [5, 6, 7, 8]).then_some(*id))
        .expect("second request should be in flight");

    driver.process_result(Deserialization::RpcResult(RpcResult {
        msg_id: second_id,
        body: vec![9, 10, 11, 12],
    }));

    assert!(first.as_mut().now_or_never().is_none());
    assert_eq!(
        second
            .as_mut()
            .now_or_never()
            .expect("matching request should finish")
            .expect("fixture response should succeed"),
        vec![9, 10, 11, 12]
    );
}

#[test]
fn retryable_bad_salt_response_is_bounded() {
    let (handle, _updates, driver) = driver_parts();
    let mut invocation = pin!(
        handle
            .invoke_raw(vec![1, 2, 3, 4])
            .expect("request should fit")
    );
    let mut driver = Box::pin(driver);

    for attempt in 0..=super::super::sender::MAX_BAD_MESSAGE_RETRIES {
        poll_driver(&mut driver);
        let request_id = *driver
            .in_flight
            .keys()
            .next()
            .expect("retry should have one in-flight request");
        driver.process_result(Deserialization::BadMessage(BadMessage {
            msg_id: request_id,
            code: 48,
        }));
        if attempt < super::super::sender::MAX_BAD_MESSAGE_RETRIES {
            assert!(invocation.as_mut().now_or_never().is_none());
        }
    }

    let error = invocation
        .as_mut()
        .now_or_never()
        .expect("retry exhaustion should complete the request")
        .expect_err("repeated bad-salt responses must fail");
    assert!(matches!(error, super::Error::BadMessage { .. }));
}

#[test]
fn connection_shutdown_completes_every_pending_invocation() {
    let (handle, _updates, driver) = driver_parts();
    let mut invocation = pin!(
        handle
            .invoke_raw(vec![1, 2, 3, 4])
            .expect("request should fit")
    );

    drop(driver);

    let error = invocation
        .as_mut()
        .now_or_never()
        .expect("driver shutdown should wake the request")
        .expect_err("stopped driver cannot complete the request");
    assert!(error.is_connection_failure());
}

#[test]
fn invocation_handle_can_stop_its_owned_driver() {
    let (handle, _updates, driver) = driver_parts();
    let mut driver = Box::pin(driver);

    handle.stop();

    let mut context = Context::from_waker(Waker::noop());
    assert!(matches!(
        driver.as_mut().poll(&mut context),
        Poll::Ready(Ok(()))
    ));
}
