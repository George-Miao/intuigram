# compio-actor

An actor framework built for [Compio](https://compio-rs). It lets you keep state
and asynchronous behavior together while the framework takes care of running each actor
on a single worker.

An `Actor` owns its state and lifecycle. It is created on its worker, initialized by
`pre_start`, and can use the remaining lifecycle hooks to start or clean up resources.
The actor, its state, and its futures stay local to that worker, so none of them need to
implement `Send`.

A `Handler<M>` teaches an actor how to process one message type. Implement it more than
once when an actor accepts different kinds of messages; all of them are handled serially
through the same bounded FIFO mailbox.

Spawning an actor gives you a `Mailbox<A>`, a typed reference that can send every message
handled by `A`. A `Broker<M>` narrows that down to a cloneable, send-only capability for
one message type, which is handy when other code should not or don't want to know the
actor's concrete type.

Messages can be sent in two ways:

- A **cast** sends a message with `Mailbox::send` or `Broker::send` and continues without
  waiting for the actor to handle it.
- A **call** sends a request with `Mailbox::call` or `Broker<Call<M, R>>::call` and waits
  for the handler to reply. The framework creates the `Call<M, R>` value and gives its
  reply capability to the handler.

A `Cluster` places actors on workers managed by `compio-dispatcher`. `Cluster::spawn`
returns a lazy builder, where you can set an optional name and mailbox capacity before
awaiting it. `ActorHandle<E>` can then be used to observe how the actor exits without
stopping it when the handle is dropped.

Anything that crosses between the cluster and an actor—factories, messages, mailboxes,
startup arguments, and errors—must implement `Send`. Calling `stop` lets the current
handler finish. The lifecycle order is `pre_start`, `post_start`, message handling,
`pre_stop`, then `post_stop`. Messages and worker-local handler futures are type-erased
internally, so each handled message currently requires two small allocations.

## Process groups

A `ProcessGroup<M>` load-balances one message type across any actors that can
produce a `Broker<M>`. `RoundRobin` is the default routing `Strategy`; pass a
custom strategy to `ProcessGroup::with_strategy` to change which member is tried
first. Routing tries each member once, skipping closed mailboxes and falling
through when a mailbox is full. The group does not keep a backlog: it returns the
original message when every member is full or no live member remains.

`ProcessGroup::join` returns a membership token. Keep that token for as long as
the actor should receive work; dropping it removes the actor from the group. A
`ProcessGroup<Call<M, R>>` can also make load-balanced calls.

## Supervision

An actor becomes a supervisor by implementing `Supervisor<Child>` and handling
the child's `ActorStarted`, `ActorTerminated`, and `ActorFailed` events. Link a
child during startup with `.supervisor(&parent)`. The link is installed before
the child's `post_start`, and lifecycle events use a dedicated channel ahead of
ordinary messages.

The default supervision handler ignores lifecycle events. Override it to stop the
parent, restart the child, or apply another policy. Stopping a parent always stops
all of its linked children, and a stopped parent rejects new supervised spawns.

## Example

```rust
use std::{convert::Infallible, io};

use compio_actor::{Actor, ActorExit, Broker, Call, Cluster, Handler, Mailbox};

struct Counter;

#[derive(Debug)]
struct Add(usize);

#[derive(Debug)]
struct Read;

#[derive(Debug)]
struct Stop;

// An actor defines the state it owns and how that state is initialized.
impl Actor for Counter {
    type State = usize;
    type Arguments = usize;
    type Error = Infallible;

    async fn pre_start(
        &self,
        _myself: &Mailbox<Self>,
        initial: usize,
    ) -> Result<usize, Infallible> {
        Ok(initial)
    }
}

// Each Handler implementation adds one message type to the actor.
impl Handler<Add> for Counter {
    async fn handle(
        &self,
        _: &Mailbox<Self>,
        Add(value): Add,
        state: &mut usize,
    ) -> Result<(), Infallible> {
        *state += value;
        Ok(())
    }
}

impl Handler<Stop> for Counter {
    async fn handle(
        &self,
        myself: &Mailbox<Self>,
        Stop: Stop,
        state: &mut usize,
    ) -> Result<(), Infallible> {
        assert_eq!(*state, 5);
        myself.stop();
        Ok(())
    }
}

impl Handler<Call<Read, usize>> for Counter {
    async fn handle(
        &self,
        _: &Mailbox<Self>,
        call: Call<Read, usize>,
        state: &mut usize,
    ) -> Result<(), Infallible> {
        call.reply(*state).ok();
        Ok(())
    }
}

fn main() -> io::Result<()> {
    compio_runtime::Runtime::new()?.block_on(async {
        // A cluster manages the workers that run actors.
        let cluster = Cluster::new()?;

        // Awaiting the spawn builder creates the actor and returns its mailbox.
        let (counter, handle) = cluster
            .spawn(|| Counter, 0)
            .name("counter")
            .await
            .unwrap();

        // A named actor can be looked up to get another mailbox.
        let counter = cluster.lookup::<Counter, _>("counter").unwrap();

        // A broker exposes only the ability to send one message type.
        let add: Broker<Add> = counter.broker();

        // Casts enqueue a message and return immediately.
        add.send(Add(2)).unwrap();
        counter.send(Add(3)).unwrap();

        // Calls wait for a response from the handler.
        assert_eq!(counter.call(Read).await.unwrap(), 5);
        counter.send(Stop).unwrap();

        // The handle reports how the actor exited.
        assert_eq!(handle.await.unwrap(), ActorExit::Stopped);
        cluster.join().await
    })
}
```
