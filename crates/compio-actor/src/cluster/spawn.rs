use std::borrow::Cow;
use std::future::{Future, IntoFuture};
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use futures_channel::oneshot;
use futures_util::FutureExt;
use snafu::Snafu;

use super::Cluster;
use crate::actor::{ActorExit, ActorHandle, finish, run};
use crate::mailbox::{DEFAULT_MAILBOX_CAPACITY, make_mailbox};
use crate::supervisor::{SupervisionTarget, Supervisor};
use crate::{Actor, Mailbox};

/// The result returned when an actor starts successfully.
pub type SpawnResult<A> =
    Result<(Mailbox<A>, ActorHandle<<A as Actor>::Error>), SpawnError<<A as Actor>::Error>>;

/// An error encountered while starting an actor.
#[derive(Debug, PartialEq, Eq, Snafu)]
#[snafu(module)]
pub enum SpawnError<E: Send + 'static> {
    /// The cluster is no longer accepting actors.
    #[snafu(display("actor cluster is unavailable"))]
    Unavailable,

    /// Another actor is registered under this name.
    #[snafu(display("actor name {name:?} is already registered"))]
    NameTaken {
        /// Rejected registry name.
        name: Cow<'static, str>,
    },

    /// The actor's startup hook failed.
    #[snafu(display("actor startup failed"))]
    Start {
        /// Actor-owned startup failure.
        error: E,
    },

    /// The worker stopped before startup completed.
    #[snafu(display("actor worker stopped during startup"))]
    WorkerStopped,

    /// The selected supervisor has stopped.
    #[snafu(display("actor supervisor has stopped"))]
    SupervisorStopped,
}

/// A configurable actor spawn operation.
#[must_use = "actors are not spawned until this builder is awaited"]
pub struct Spawn<'a, A, F>
where
    A: Actor,
    F: FnOnce() -> A + Send + 'static,
{
    cluster: &'a Cluster,
    factory: F,
    arguments: A::Arguments,
    name: Option<Cow<'static, str>>,
    capacity: NonZeroUsize,
    supervisor: Option<SupervisionTarget<A>>,
}

impl<'a, A, F> Spawn<'a, A, F>
where
    A: Actor,
    F: FnOnce() -> A + Send + 'static,
{
    pub(super) fn new(cluster: &'a Cluster, factory: F, arguments: A::Arguments) -> Self {
        Self {
            cluster,
            factory,
            arguments,
            name: None,
            capacity: DEFAULT_MAILBOX_CAPACITY,
            supervisor: None,
        }
    }

    /// Registers the actor under `name` after startup succeeds.
    pub fn name(mut self, name: impl Into<Cow<'static, str>>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the actor's bounded mailbox capacity.
    pub fn capacity(mut self, capacity: NonZeroUsize) -> Self {
        self.capacity = capacity;
        self
    }

    /// Links the actor to a supervisor for its whole lifetime.
    pub fn supervisor<S>(mut self, supervisor: &Mailbox<S>) -> Self
    where
        S: Supervisor<A>,
    {
        self.supervisor = Some(SupervisionTarget::new(supervisor));
        self
    }
}

impl<A, F> IntoFuture for Spawn<'_, A, F>
where
    A: Actor,
    F: FnOnce() -> A + Send + 'static,
{
    type IntoFuture = SpawnFuture<A>;
    type Output = SpawnResult<A>;

    fn into_future(self) -> Self::IntoFuture {
        self.cluster.start(
            self.factory,
            self.arguments,
            self.name,
            self.capacity,
            self.supervisor,
        )
    }
}

/// The future produced by [`Spawn`].
pub enum SpawnFuture<A: Actor> {
    #[doc(hidden)]
    Ready(Option<SpawnResult<A>>),
    #[doc(hidden)]
    Pending {
        mailbox: Mailbox<A>,
        result: oneshot::Receiver<Result<ActorExit<A::Error>, ()>>,
        started: oneshot::Receiver<Result<(), A::Error>>,
    },
}

impl<A: Actor> SpawnFuture<A> {
    fn ready(result: SpawnResult<A>) -> Self {
        Self::Ready(Some(result))
    }

    fn pending(
        mailbox: Mailbox<A>,
        result: oneshot::Receiver<Result<ActorExit<A::Error>, ()>>,
        started: oneshot::Receiver<Result<(), A::Error>>,
    ) -> Self {
        Self::Pending {
            mailbox,
            result,
            started,
        }
    }
}

impl<A: Actor> Future for SpawnFuture<A> {
    type Output = SpawnResult<A>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let started = match this {
            Self::Ready(result) => {
                return Poll::Ready(result.take().expect("spawn future polled after completion"));
            }
            Self::Pending { started, .. } => ready!(started.poll_unpin(cx)),
        };
        let (mailbox, result) = match std::mem::replace(this, Self::Ready(None)) {
            Self::Pending {
                mailbox, result, ..
            } => (mailbox, result),
            Self::Ready(_) => unreachable!(),
        };
        Poll::Ready(match started {
            Ok(Ok(())) => Ok((mailbox, ActorHandle { result })),
            Ok(Err(error)) => Err(SpawnError::Start { error }),
            Err(_) => Err(SpawnError::WorkerStopped),
        })
    }
}

impl<A: Actor> Unpin for SpawnFuture<A> {}

impl Cluster {
    fn start<A, F>(
        &self,
        factory: F,
        arguments: A::Arguments,
        name: Option<Cow<'static, str>>,
        capacity: NonZeroUsize,
        supervisor: Option<SupervisionTarget<A>>,
    ) -> SpawnFuture<A>
    where
        A: Actor,
        F: FnOnce() -> A + Send + 'static,
    {
        let reg = match name {
            Some(name) => match self.registry.reserve(name) {
                Ok(registration) => Some(registration),
                Err(name) => {
                    return SpawnFuture::ready(Err(SpawnError::NameTaken { name }));
                }
            },
            None => None,
        };

        let (mailbox, receiver) = make_mailbox::<A>(capacity);
        let supervision = match supervisor {
            Some(supervisor) => match supervisor.link(&mailbox) {
                Ok(supervision) => Some(supervision),
                Err(_) => return SpawnFuture::ready(Err(SpawnError::SupervisorStopped)),
            },
            None => None,
        };
        let actor_ref = mailbox.clone();
        let (started_tx, started_rx) = oneshot::channel();
        let result = match self.dispatcher.dispatch(move || async move {
            let mut reg = reg;
            let actor = factory();
            let mut state = match actor.pre_start(&actor_ref, arguments).await {
                Ok(state) => state,
                Err(error) => {
                    reg.take();
                    started_tx.send(Err(error)).ok();
                    return Err(());
                }
            };
            if let Some(registration) = &reg {
                registration.activate(&actor_ref);
            }

            if started_tx.send(Ok(())).is_err() {
                let exit =
                    finish(&actor, &actor_ref, receiver, &mut state, ActorExit::Stopped).await;
                return Ok(exit);
            }

            Ok(run(actor, actor_ref, receiver, state, supervision).await)
        }) {
            Ok(result) => result,
            Err(_) => return SpawnFuture::ready(Err(SpawnError::Unavailable)),
        };

        SpawnFuture::pending(mailbox, result, started_rx)
    }
}
