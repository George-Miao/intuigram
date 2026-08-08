use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use flume::Receiver as FlumeReceiver;
use futures_util::{FutureExt, pin_mut, select_biased};

use super::{Mailbox, MailboxInner};
use crate::Actor;
use crate::actor::Delivering;
use crate::supervisor::Children;

pub(crate) struct Receiver<A: Actor> {
    messages: FlumeReceiver<Delivering<A>>,
    stop: FlumeReceiver<()>,
    supervision: FlumeReceiver<Delivering<A>>,
}

impl<A: Actor> Receiver<A> {
    pub(crate) async fn recv(&self) -> MailboxEvent<A> {
        let stop = self.stop.recv_async().fuse();
        let supervision = self.supervision.recv_async().fuse();
        let message = self.messages.recv_async().fuse();
        pin_mut!(stop, supervision, message);

        select_biased! {
            _ = stop => MailboxEvent::Stop,
            supervision = supervision => match supervision {
                Ok(supervision) => MailboxEvent::Supervision(supervision),
                Err(_) => MailboxEvent::Stop,
            },
            message = message => match message {
                Ok(message) => MailboxEvent::Message(message),
                Err(_) => MailboxEvent::Stop,
            },
        }
    }
}

pub(crate) enum MailboxEvent<A: Actor> {
    Message(Delivering<A>),
    Supervision(Delivering<A>),
    Stop,
}

pub(crate) fn make_mailbox<A: Actor>(capacity: NonZeroUsize) -> (Mailbox<A>, Receiver<A>) {
    let (message_tx, message_rx) = flume::bounded(capacity.get());
    let (stop_tx, stop_rx) = flume::bounded(1);
    let (supervision_tx, supervision_rx) = flume::unbounded();
    let inner = Arc::new(MailboxInner {
        messages: message_tx,
        stop: stop_tx,
        supervision: supervision_tx,
        children: Children::default(),
        stopping: AtomicBool::new(false),
        capacity,
    });

    (
        Mailbox { inner },
        Receiver {
            messages: message_rx,
            stop: stop_rx,
            supervision: supervision_rx,
        },
    )
}
