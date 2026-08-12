use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use compio::actor::Mailbox;
use intuigram_lib::AdapterEvent;
use intuigram_store::{AccountStore, OutboxRecord};
use snafu::ResultExt;

use super::super::{BackendOutput, Error, OperationProviderSnafu, Result, outbox_view};
use super::actor::TelegramActor;

mod actor;
mod policy;
mod step;
#[cfg(test)]
mod tests;

use actor::{ExecuteOutbox, OutboxResponse};

const BUSY_RETRY: Duration = Duration::from_millis(50);

type Pending = Pin<Box<dyn Future<Output = Result<Advance>>>>;

enum Advance {
    Claim(step::Claim),
    Outcome(step::Outcome),
    Continue,
}

pub(super) struct Coordinator {
    store: AccountStore,
    mailbox: Mailbox<TelegramActor>,
    pending: Option<Pending>,
    outputs: VecDeque<BackendOutput>,
    error: Option<Error>,
    poll_requested: bool,
    waker: Option<Waker>,
}

impl Coordinator {
    pub(super) fn new(store: AccountStore, mailbox: Mailbox<TelegramActor>) -> Self {
        Self {
            store,
            mailbox,
            pending: None,
            outputs: VecDeque::new(),
            error: None,
            poll_requested: true,
            waker: None,
        }
    }

    pub(super) fn wake(&mut self) {
        self.poll_requested = true;
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }

    pub(super) fn poll(
        &mut self,
        providers: &RefCell<crate::OperationProviders>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<BackendOutput>> {
        if let Some(output) = self.outputs.pop_front() {
            return Poll::Ready(Ok(output));
        }
        if let Some(error) = self.error.take() {
            return Poll::Ready(Err(error));
        }
        loop {
            if self.pending.is_none() {
                if !self.poll_requested {
                    self.park(cx);
                    return Poll::Pending;
                }
                self.poll_requested = false;
                let now = match providers.borrow().now().context(OperationProviderSnafu) {
                    Ok(now) => now,
                    Err(error) => return Poll::Ready(Err(error)),
                };
                let store = self.store.clone();
                self.pending = Some(Box::pin(async move {
                    step::claim(store, now).await.map(Advance::Claim)
                }));
            }
            let future = self
                .pending
                .as_mut()
                .expect("a missing Outbox future is created before polling");
            let advance = match future.as_mut().poll(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Ok(advance)) => advance,
                Poll::Ready(Err(error)) => {
                    self.pending = None;
                    return Poll::Ready(Err(error));
                }
            };
            self.pending = None;
            match advance {
                Advance::Claim(claim) => {
                    self.outputs.extend(claim.expired.into_iter().map(|record| {
                        BackendOutput::event(Some(AdapterEvent::OutboxChanged(outbox_view(record))))
                    }));
                    match claim.head {
                        step::Head::Claimed(record) => {
                            self.start_execution(providers, record.clone())?;
                            self.outputs.push_back(BackendOutput::event(Some(
                                AdapterEvent::OutboxChanged(outbox_view(record)),
                            )));
                        }
                        step::Head::WaitingUntil(available_at) => {
                            self.start_wait(providers, available_at)?;
                        }
                        step::Head::Busy => self.start_delay(BUSY_RETRY),
                        step::Head::Idle => {}
                    }
                    if let Some(output) = self.outputs.pop_front() {
                        return Poll::Ready(Ok(output));
                    }
                    if self.pending.is_none() {
                        self.park(cx);
                        return Poll::Pending;
                    }
                }
                Advance::Outcome(outcome) => {
                    self.outputs.extend(outcome.outputs);
                    self.error = outcome.reconnect.map(|source| Error::Telegram { source });
                    self.poll_requested = true;
                    if let Some(output) = self.outputs.pop_front() {
                        return Poll::Ready(Ok(output));
                    }
                }
                Advance::Continue => self.poll_requested = true,
            }
        }
    }

    fn park(&mut self, cx: &Context<'_>) {
        if self
            .waker
            .as_ref()
            .is_none_or(|waker| !waker.will_wake(cx.waker()))
        {
            self.waker = Some(cx.waker().clone());
        }
    }

    fn start_execution(
        &mut self,
        providers: &RefCell<crate::OperationProviders>,
        record: OutboxRecord,
    ) -> Result<()> {
        let now = providers
            .borrow()
            .replay(record_random_id(&record))
            .context(OperationProviderSnafu)?
            .observed_at();
        let store = self.store.clone();
        let mailbox = self.mailbox.clone();
        self.pending = Some(Box::pin(async move {
            step::execute(store, mailbox, record, now)
                .await
                .map(Advance::Outcome)
        }));
        Ok(())
    }

    fn start_wait(
        &mut self,
        providers: &RefCell<crate::OperationProviders>,
        available_at: i64,
    ) -> Result<()> {
        let now = providers.borrow().now().context(OperationProviderSnafu)?;
        self.start_delay(wait_duration(now, available_at));
        Ok(())
    }

    fn start_delay(&mut self, duration: Duration) {
        self.pending = Some(Box::pin(async move {
            compio::time::sleep(duration).await;
            Ok(Advance::Continue)
        }));
    }
}

fn record_random_id(record: &OutboxRecord) -> i64 {
    let intuigram_store::OutboxPayload::V1(payload) = &record.payload;
    payload.random_id
}

fn wait_duration(now: i64, available_at: i64) -> Duration {
    let seconds = available_at.saturating_sub(now);
    if seconds <= 0 {
        Duration::ZERO
    } else {
        Duration::from_secs(seconds as u64)
    }
}
