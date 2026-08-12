use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicI64, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;

use grammers_mtproto::mtp::Encrypted;
use grammers_tl_types::{Serializable as _, functions};

use crate::sender::{EncodedEnvelope, Result, encode_envelope};

const DISCONNECT_DELAY_SECONDS: i32 = 75;
static NEXT_PING_ID: AtomicI64 = AtomicI64::new(1);

pub(super) struct Keepalive {
    delay: Duration,
    timer: Option<compio::time::Sleep>,
    pending: Option<Vec<u8>>,
}

impl Keepalive {
    pub(super) const fn new(delay: Duration) -> Self {
        Self {
            delay,
            timer: None,
            pending: None,
        }
    }

    pub(super) fn poll(&mut self, cx: &mut Context<'_>) {
        let timer = self
            .timer
            .get_or_insert_with(|| compio::time::sleep(self.delay));
        if Pin::new(timer).poll(cx) == Poll::Pending {
            return;
        }
        self.timer = Some(compio::time::sleep(self.delay));
        self.pending.get_or_insert_with(|| {
            functions::PingDelayDisconnect {
                ping_id: NEXT_PING_ID.fetch_add(1, Ordering::Relaxed),
                disconnect_delay: DISCONNECT_DELAY_SECONDS,
            }
            .to_bytes()
        });
    }

    pub(super) const fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub(super) fn prepare(&mut self, mtp: &mut Encrypted) -> Result<Option<Vec<u8>>> {
        let Some(body) = self.pending.as_ref() else {
            return Ok(None);
        };
        match encode_envelope(mtp, body)? {
            EncodedEnvelope::Request { payload, .. } => {
                self.pending = None;
                Ok(Some(payload))
            }
            EncodedEnvelope::Service(payload) => Ok(Some(payload)),
            EncodedEnvelope::AwaitingService => Ok(None),
        }
    }
}
