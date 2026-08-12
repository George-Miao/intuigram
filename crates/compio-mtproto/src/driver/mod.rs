//! Persistent single-owner MTProto connection driver.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use grammers_mtproto::MsgId;
use grammers_mtproto::mtp::{Deserialization, Encrypted, Mtp};
use snafu::ResultExt;

use crate::BoxedTransport;
use crate::sender::{
    BadMessageSnafu, DeserializeEnvelopeSnafu, DriverStoppedSnafu, EncodedEnvelope, Error,
    ResponseFailureSnafu, Result, RpcSnafu, encode_envelope, finalize_service_envelope,
};

mod invocation;
mod keepalive;

pub use invocation::{Invocation, InvocationHandle, UpdateStream};
use keepalive::Keepalive;

struct QueuedRequest {
    body: Vec<u8>,
    attempts: usize,
    response: Rc<Response>,
}

#[derive(Debug)]
struct Response {
    result: RefCell<Option<Result<Vec<u8>>>>,
    waker: RefCell<Option<Waker>>,
    completed: Cell<bool>,
}

impl Response {
    fn new() -> Self {
        Self {
            result: RefCell::new(None),
            waker: RefCell::new(None),
            completed: Cell::new(false),
        }
    }

    fn finish(&self, result: Result<Vec<u8>>) {
        *self.result.borrow_mut() = Some(result);
        if let Some(waker) = self.waker.borrow_mut().take() {
            waker.wake();
        }
    }
}

struct Shared {
    capacity: usize,
    outstanding: Cell<usize>,
    requests: RefCell<VecDeque<QueuedRequest>>,
    updates: RefCell<VecDeque<Vec<u8>>>,
    driver_waker: RefCell<Option<Waker>>,
    update_waker: RefCell<Option<Waker>>,
    stopped: Cell<bool>,
}

impl Shared {
    fn new(capacity: NonZeroUsize) -> Rc<Self> {
        Rc::new(Self {
            capacity: capacity.get(),
            outstanding: Cell::new(0),
            requests: RefCell::new(VecDeque::with_capacity(capacity.get())),
            updates: RefCell::new(VecDeque::new()),
            driver_waker: RefCell::new(None),
            update_waker: RefCell::new(None),
            stopped: Cell::new(false),
        })
    }

    fn enqueue(self: &Rc<Self>, body: Vec<u8>) -> Result<Invocation> {
        if self.stopped.get() {
            return DriverStoppedSnafu.fail();
        }
        if self.outstanding.get() >= self.capacity {
            return Err(Error::QueueFull {
                capacity: self.capacity,
            });
        }
        let response = Rc::new(Response::new());
        self.requests.borrow_mut().push_back(QueuedRequest {
            body,
            attempts: 0,
            response: Rc::clone(&response),
        });
        self.outstanding.set(self.outstanding.get() + 1);
        if let Some(waker) = self.driver_waker.borrow_mut().take() {
            waker.wake();
        }
        Ok(Invocation {
            response,
            shared: Rc::clone(self),
        })
    }

    fn complete(&self, response: &Response, result: Result<Vec<u8>>) {
        if response.completed.replace(true) {
            return;
        }
        self.outstanding
            .set(self.outstanding.get().saturating_sub(1));
        response.finish(result);
    }

    fn publish_update(&self, update: Vec<u8>) {
        self.updates.borrow_mut().push_back(update);
        if let Some(waker) = self.update_waker.borrow_mut().take() {
            waker.wake();
        }
    }

    fn stop(&self) {
        if self.stopped.replace(true) {
            return;
        }
        for request in self.requests.borrow_mut().drain(..) {
            self.complete(&request.response, Err(DriverStoppedSnafu.build()));
        }
        if let Some(waker) = self.update_waker.borrow_mut().take() {
            waker.wake();
        }
    }
}

/// Persistent single-owner MTProto network driver.
pub struct ConnectionDriver {
    transport: BoxedTransport,
    mtp: Encrypted,
    shared: Rc<Shared>,
    pending: Option<QueuedRequest>,
    retries: VecDeque<QueuedRequest>,
    in_flight: HashMap<MsgId, QueuedRequest>,
    outgoing: VecDeque<Vec<u8>>,
    sending: bool,
    keepalive: Keepalive,
}

impl ConnectionDriver {
    fn prepare_outgoing(&mut self) -> Result<bool> {
        if !self.outgoing.is_empty() || self.sending {
            return Ok(false);
        }
        if self.pending.is_none() && self.keepalive.is_pending() {
            let Some(payload) = self.keepalive.prepare(&mut self.mtp)? else {
                return Ok(false);
            };
            self.outgoing.push_back(payload);
            return Ok(true);
        }
        while self.pending.is_none() {
            let next = self
                .retries
                .pop_front()
                .or_else(|| self.shared.requests.borrow_mut().pop_front());
            let Some(next) = next else {
                break;
            };
            if !next.response.completed.get() {
                self.pending = Some(next);
            }
        }
        let Some(request) = self.pending.as_ref() else {
            if let Some(service) = finalize_service_envelope(&mut self.mtp) {
                self.outgoing.push_back(service);
                return Ok(true);
            }
            return Ok(false);
        };
        match encode_envelope(&mut self.mtp, &request.body)? {
            EncodedEnvelope::Request {
                request_id,
                payload,
            } => {
                let request = self
                    .pending
                    .take()
                    .expect("an accepted request remains pending until it receives an ID");
                self.in_flight.insert(request_id, request);
                self.outgoing.push_back(payload);
                Ok(true)
            }
            EncodedEnvelope::Service(payload) => {
                self.outgoing.push_back(payload);
                Ok(true)
            }
            EncodedEnvelope::AwaitingService => Ok(false),
        }
    }

    fn process_envelope(&mut self, mut envelope: Vec<u8>) -> Result<()> {
        let results = self
            .mtp
            .deserialize(&mut envelope)
            .context(DeserializeEnvelopeSnafu)?;
        if let Some(service) = finalize_service_envelope(&mut self.mtp) {
            self.outgoing.push_back(service);
        }
        for result in results {
            self.process_result(result);
        }
        Ok(())
    }

    fn process_result(&mut self, result: Deserialization) {
        match result {
            Deserialization::OwnUpdate { update, .. } | Deserialization::Update(update) => {
                self.shared.publish_update(update);
            }
            Deserialization::RpcResult(result) => {
                if let Some(request) = self.in_flight.remove(&result.msg_id) {
                    self.shared.complete(&request.response, Ok(result.body));
                }
            }
            Deserialization::RpcError(result) => {
                if let Some(request) = self.in_flight.remove(&result.msg_id) {
                    self.shared.complete(
                        &request.response,
                        Err(RpcSnafu {
                            code: result.error.error_code,
                            message: result.error.error_message,
                        }
                        .build()),
                    );
                }
            }
            Deserialization::BadMessage(result) => {
                let Some(mut request) = self.in_flight.remove(&result.msg_id) else {
                    return;
                };
                if request.response.completed.get() {
                    return;
                }
                if result.retryable() && request.attempts < super::sender::MAX_BAD_MESSAGE_RETRIES {
                    request.attempts += 1;
                    self.retries.push_back(request);
                } else {
                    self.shared.complete(
                        &request.response,
                        Err(BadMessageSnafu {
                            description: result.description(),
                        }
                        .build()),
                    );
                }
            }
            Deserialization::Failure(result) => {
                if let Some(request) = self.in_flight.remove(&result.msg_id) {
                    self.shared
                        .complete(&request.response, Err(ResponseFailureSnafu.build()));
                }
            }
        }
    }

    fn stop_pending(&mut self) {
        if let Some(request) = self.pending.take() {
            self.shared
                .complete(&request.response, Err(DriverStoppedSnafu.build()));
        }
        for request in self.retries.drain(..) {
            self.shared
                .complete(&request.response, Err(DriverStoppedSnafu.build()));
        }
        for (_, request) in self.in_flight.drain() {
            self.shared
                .complete(&request.response, Err(DriverStoppedSnafu.build()));
        }
        self.shared.stop();
    }
}

impl Future for ConnectionDriver {
    type Output = Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        *self.shared.driver_waker.borrow_mut() = Some(cx.waker().clone());
        if self.shared.stopped.get() {
            self.stop_pending();
            return Poll::Ready(Ok(()));
        }
        self.keepalive.poll(cx);
        loop {
            if self.shared.stopped.get() {
                self.stop_pending();
                return Poll::Ready(Ok(()));
            }
            if let Err(error) = self.prepare_outgoing() {
                self.stop_pending();
                return Poll::Ready(Err(error));
            }
            if !self.sending
                && let Some(payload) = self.outgoing.pop_front()
            {
                self.transport.queue_send(payload);
                self.sending = true;
            }
            if self.sending {
                match self.transport.poll_send(cx) {
                    Poll::Ready(Ok(())) => {
                        self.sending = false;
                        continue;
                    }
                    Poll::Ready(Err(source)) => {
                        self.stop_pending();
                        return Poll::Ready(Err(Error::Transport { source }));
                    }
                    Poll::Pending => {}
                }
            }
            match self.transport.poll_receive(cx) {
                Poll::Ready(Ok(envelope)) => {
                    if let Err(error) = self.process_envelope(envelope) {
                        self.stop_pending();
                        return Poll::Ready(Err(error));
                    }
                    continue;
                }
                Poll::Ready(Err(source)) => {
                    self.stop_pending();
                    return Poll::Ready(Err(Error::Transport { source }));
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl Drop for ConnectionDriver {
    fn drop(&mut self) {
        self.stop_pending();
    }
}

pub(crate) fn from_parts(
    transport: BoxedTransport,
    mtp: Encrypted,
    pending_updates: Vec<Vec<u8>>,
    capacity: NonZeroUsize,
) -> (InvocationHandle, UpdateStream, ConnectionDriver) {
    let shared = Shared::new(capacity);
    for update in pending_updates {
        shared.publish_update(update);
    }
    (
        InvocationHandle {
            shared: Rc::clone(&shared),
        },
        UpdateStream {
            shared: Rc::clone(&shared),
        },
        ConnectionDriver {
            transport,
            mtp,
            shared,
            pending: None,
            retries: VecDeque::new(),
            in_flight: HashMap::new(),
            outgoing: VecDeque::new(),
            sending: false,
            keepalive: Keepalive::new(Duration::from_secs(60)),
        },
    )
}

#[cfg(test)]
mod tests;
