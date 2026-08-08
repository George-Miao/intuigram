//! Typed actor groups with configurable routing.

use std::fmt;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex, Weak};

use crate::mailbox::{CallError, DeliverError, call_with};
use crate::{Broker, Call, Message};

/// Selects the first member considered for each routed message.
pub trait Strategy: Send + 'static {
    /// Returns the preferred member index.
    fn select(&mut self, members: NonZeroUsize) -> usize;
}

/// Routes messages round-robin across group members.
#[derive(Clone, Copy, Debug, Default)]
pub struct RoundRobin {
    next: usize,
}

impl RoundRobin {
    /// Creates a round-robin strategy starting at the first member.
    pub const fn new() -> Self {
        Self { next: 0 }
    }
}

impl Strategy for RoundRobin {
    fn select(&mut self, members: NonZeroUsize) -> usize {
        let selected = self.next % members.get();
        self.next = self.next.wrapping_add(1);
        selected
    }
}

/// A group of actors that share messages using `S`.
pub struct ProcessGroup<M: Message, S: Strategy = RoundRobin> {
    inner: Arc<GroupInner<M, S>>,
}

impl<M: Message> ProcessGroup<M, RoundRobin> {
    /// Creates an empty process group.
    pub fn new() -> Self {
        Self::with_strategy(RoundRobin::new())
    }
}

impl<M: Message, S: Strategy> ProcessGroup<M, S> {
    /// Creates an empty process group with a routing strategy.
    pub fn with_strategy(strategy: S) -> Self {
        Self {
            inner: Arc::new(GroupInner {
                state: Mutex::new(GroupState {
                    next_id: 0,
                    members: Vec::new(),
                    strategy,
                }),
            }),
        }
    }

    /// Adds a broker until the returned membership is dropped.
    pub fn join(&self, broker: Broker<M>) -> Membership<M, S> {
        let mut state = self
            .inner
            .state
            .lock()
            .expect("process-group lock poisoning is unrecoverable");
        let id = state.next_id;
        state.next_id = state.next_id.wrapping_add(1);
        state.members.push(Member { id, broker });
        Membership {
            id,
            group: Arc::downgrade(&self.inner),
        }
    }

    /// Routes a message to the next available member.
    pub fn send(&self, mut message: M) -> Result<(), DeliverError<M>> {
        let mut state = self
            .inner
            .state
            .lock()
            .expect("process-group lock poisoning is unrecoverable");
        let attempts = state.members.len();
        let mut attempted = 0;
        let mut saw_full = false;
        let mut index = match NonZeroUsize::new(state.members.len()) {
            Some(members) => state.strategy.select(members) % members.get(),
            None => return Err(DeliverError::Closed { message }),
        };

        while attempted < attempts && !state.members.is_empty() {
            attempted += 1;

            match state.members[index].broker.send(message) {
                Ok(()) => return Ok(()),
                Err(DeliverError::Full { message: returned }) => {
                    saw_full = true;
                    message = returned;
                    index = (index + 1) % state.members.len();
                }
                Err(DeliverError::Closed { message: returned }) => {
                    message = returned;
                    state.members.remove(index);
                    if !state.members.is_empty() {
                        index %= state.members.len();
                    }
                }
            }
        }

        if saw_full {
            Err(DeliverError::Full { message })
        } else {
            Err(DeliverError::Closed { message })
        }
    }

    /// Returns the number of registered members.
    pub fn len(&self) -> usize {
        self.inner
            .state
            .lock()
            .expect("process-group lock poisoning is unrecoverable")
            .members
            .len()
    }

    /// Returns whether the group has no members.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<M: Message, R: Message, S: Strategy> ProcessGroup<Call<M, R>, S> {
    /// Routes a request and waits for the selected actor's reply.
    pub async fn call(&self, message: M) -> Result<R, CallError<M>> {
        call_with(message, |call| self.send(call)).await
    }
}

impl<M: Message, S: Strategy> Clone for ProcessGroup<M, S> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<M: Message, S> Default for ProcessGroup<M, S>
where
    S: Strategy + Default,
{
    fn default() -> Self {
        Self::with_strategy(S::default())
    }
}

impl<M: Message, S: Strategy> fmt::Debug for ProcessGroup<M, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProcessGroup")
            .field("members", &self.len())
            .finish()
    }
}

/// An actor's membership in a [`ProcessGroup`].
#[must_use = "dropping the membership removes the actor from the process group"]
pub struct Membership<M: Message, S: Strategy = RoundRobin> {
    id: u64,
    group: Weak<GroupInner<M, S>>,
}

impl<M: Message, S: Strategy> Membership<M, S> {
    /// Removes the actor from the group.
    pub fn leave(self) {}
}

impl<M: Message, S: Strategy> Drop for Membership<M, S> {
    fn drop(&mut self) {
        let Some(group) = self.group.upgrade() else {
            return;
        };
        let mut state = group
            .state
            .lock()
            .expect("process-group lock poisoning is unrecoverable");
        if let Some(index) = state.members.iter().position(|member| member.id == self.id) {
            state.members.remove(index);
        }
    }
}

impl<M: Message, S: Strategy> fmt::Debug for Membership<M, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Membership").field("id", &self.id).finish()
    }
}

struct GroupInner<M: Message, S: Strategy> {
    state: Mutex<GroupState<M, S>>,
}

struct GroupState<M: Message, S: Strategy> {
    next_id: u64,
    members: Vec<Member<M>>,
    strategy: S,
}

struct Member<M: Message> {
    id: u64,
    broker: Broker<M>,
}
