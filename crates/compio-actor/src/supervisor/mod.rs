//! Parent-child actor supervision.

use std::any::Any;
use std::fmt;
use std::sync::{Arc, Mutex, OnceLock, Weak};

use snafu::Snafu;

use crate::actor::{Deliverable, Delivering, DeliveryFuture};
use crate::mailbox::MailboxInner;
use crate::{Actor, Mailbox};

/// A lifecycle event emitted by a supervised actor.
pub enum SupervisionEvent<A: Actor> {
    /// The actor completed its startup hooks.
    ActorStarted(Mailbox<A>),
    /// The actor stopped normally.
    ActorTerminated(Mailbox<A>),
    /// The actor exited after a lifecycle or handler error.
    ActorFailed(Mailbox<A>),
}

impl<A: Actor> SupervisionEvent<A> {
    /// Returns the actor that emitted this event.
    pub fn actor(&self) -> &Mailbox<A> {
        match self {
            Self::ActorStarted(actor) | Self::ActorTerminated(actor) | Self::ActorFailed(actor) => {
                actor
            }
        }
    }
}

impl<A: Actor> fmt::Debug for SupervisionEvent<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ActorStarted(actor) => f.debug_tuple("ActorStarted").field(actor).finish(),
            Self::ActorTerminated(actor) => f.debug_tuple("ActorTerminated").field(actor).finish(),
            Self::ActorFailed(actor) => f.debug_tuple("ActorFailed").field(actor).finish(),
        }
    }
}

/// Handles lifecycle events from actors of type `A`.
#[allow(async_fn_in_trait)]
pub trait Supervisor<A: Actor>: Actor {
    /// Handles one child lifecycle event at a time.
    async fn handle_supervision(
        &self,
        _myself: &Mailbox<Self>,
        _event: SupervisionEvent<A>,
        _state: &mut Self::State,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

struct Event<A: Actor>(SupervisionEvent<A>);

impl<S, A> Deliverable<S> for Event<A>
where
    S: Supervisor<A>,
    A: Actor,
{
    fn deliver_to<'a>(
        self: Box<Self>,
        actor: &'a S,
        myself: &'a Mailbox<S>,
        state: &'a mut S::State,
    ) -> DeliveryFuture<'a, S::Error> {
        let Self(event) = *self;
        Box::pin(async move { actor.handle_supervision(myself, event, state).await })
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any + Send> {
        self
    }
}

pub(crate) trait ActorControl: Send + Sync {
    fn stop(&self) -> bool;
}

#[derive(Debug, Snafu)]
#[snafu(display("the supervising actor is closed"))]
pub(crate) struct LinkError;

type LinkResult<T> = std::result::Result<T, LinkError>;

trait SupervisionSink<A: Actor>: Send + Sync {
    fn is_closed(&self) -> bool;
    fn link(&self, actor: Arc<dyn ActorControl>) -> LinkResult<ChildLink>;
    fn notify(&self, event: SupervisionEvent<A>);
}

impl<S, A> SupervisionSink<A> for MailboxInner<S>
where
    S: Supervisor<A>,
    A: Actor,
{
    fn is_closed(&self) -> bool {
        self.is_closed()
    }

    fn link(&self, actor: Arc<dyn ActorControl>) -> LinkResult<ChildLink> {
        if self.is_closed() {
            return Err(LinkError);
        }
        let link = self.children.link(actor.clone());
        if self.is_closed() {
            drop(link);
            actor.stop();
            return Err(LinkError);
        }
        Ok(link)
    }

    fn notify(&self, event: SupervisionEvent<A>) {
        self.supervision.send(Delivering::from(Event(event))).ok();
    }
}

pub(crate) struct SupervisionTarget<A: Actor> {
    supervisor: Weak<dyn SupervisionSink<A>>,
}

impl<A: Actor> SupervisionTarget<A> {
    pub(crate) fn new<S>(supervisor: &Mailbox<S>) -> Self
    where
        S: Supervisor<A>,
    {
        let supervisor: Arc<dyn SupervisionSink<A>> = supervisor.inner.clone();
        Self {
            supervisor: Arc::downgrade(&supervisor),
        }
    }

    pub(crate) fn link(self, actor: &Mailbox<A>) -> LinkResult<Supervision<A>> {
        let supervisor = self.supervisor.upgrade().ok_or(LinkError)?;
        if supervisor.is_closed() {
            return Err(LinkError);
        }
        let link = supervisor.link(actor.control())?;
        Ok(Supervision {
            supervisor: Arc::downgrade(&supervisor),
            link,
        })
    }
}

pub(crate) struct Supervision<A: Actor> {
    supervisor: Weak<dyn SupervisionSink<A>>,
    link: ChildLink,
}

impl<A: Actor> Supervision<A> {
    pub(crate) fn started(&self, actor: &Mailbox<A>) {
        if let Some(supervisor) = self.supervisor.upgrade() {
            supervisor.notify(SupervisionEvent::ActorStarted(actor.clone()));
        }
    }

    pub(crate) fn terminated(&self, actor: &Mailbox<A>) {
        if let Some(supervisor) = self.supervisor.upgrade() {
            supervisor.notify(SupervisionEvent::ActorTerminated(actor.clone()));
        }
    }

    pub(crate) fn failed(&self, actor: &Mailbox<A>) {
        if let Some(supervisor) = self.supervisor.upgrade() {
            supervisor.notify(SupervisionEvent::ActorFailed(actor.clone()));
        }
    }
}

impl<A: Actor> Drop for Supervision<A> {
    fn drop(&mut self) {
        self.link.unlink();
    }
}

#[derive(Default)]
pub(crate) struct Children(OnceLock<Arc<ChildSet>>);

impl Children {
    fn link(&self, actor: Arc<dyn ActorControl>) -> ChildLink {
        let children = self.0.get_or_init(|| Arc::new(ChildSet::default()));
        let mut state = children
            .state
            .lock()
            .expect("supervisor child lock poisoning is unrecoverable");
        let id = state.next_id;
        state.next_id = state.next_id.wrapping_add(1);
        state.children.push((id, actor));
        ChildLink {
            id,
            children: Arc::downgrade(children),
        }
    }

    pub(crate) fn stop_all(&self) {
        let Some(children) = self.0.get() else {
            return;
        };
        let actors = {
            let mut state = children
                .state
                .lock()
                .expect("supervisor child lock poisoning is unrecoverable");
            std::mem::take(&mut state.children)
        };
        for (_, actor) in actors {
            actor.stop();
        }
    }
}

#[derive(Default)]
struct ChildSet {
    state: Mutex<ChildState>,
}

#[derive(Default)]
struct ChildState {
    next_id: u64,
    children: Vec<(u64, Arc<dyn ActorControl>)>,
}

struct ChildLink {
    id: u64,
    children: Weak<ChildSet>,
}

impl ChildLink {
    fn unlink(&self) {
        let Some(children) = self.children.upgrade() else {
            return;
        };
        let mut state = children
            .state
            .lock()
            .expect("supervisor child lock poisoning is unrecoverable");
        if let Some(index) = state.children.iter().position(|(id, _)| *id == self.id) {
            state.children.swap_remove(index);
        }
    }
}
