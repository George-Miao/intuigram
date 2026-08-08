# Compio actor isolation viability

Research snapshot: 2026-08-08

## Question

Can Intuigram use the experimental `compio-actor` checkout at
`/Users/pop/Dev/Org/Compio/compio-actor` to move Telegram and terminal work to
actors on other threads, then aggregate their outputs on the main application
thread?

## Verdict

**Conditional go for a Telegram-actor prototype; no-go for moving the terminal
or adopting the framework in production in its current checkout.** A dedicated
Telegram worker is a promising way to remove Telegram connection setup, RPCs,
flood waits, and update normalization from the UI/application thread. It does
not by itself remove Intuigram's head-of-line blocking: the main loop currently
allows only one active `Backend` future, and a `compio-actor` actor also awaits
one handler at a time. The migration must therefore split the monolithic
backend, keep several correlated adapter operations in flight, and run the
MTProto connection driver as a separate worker-local task
([current runtime loop](../../crates/intuigram/src/application/runtime_loop.rs),
[actor delivery loop](../../../../Org/Compio/compio-actor/src/actor/deliver.rs)).

Keep terminal input, rendering, the `intuigram-app` reducer, and result
aggregation on the process's main Compio thread. There is no demonstrated hard
Rust or Crossterm rule that a terminal must run on the main OS thread, but
moving it provides little isolation, crosses large `View` snapshots between
threads, complicates the one-reader terminal session, and adds an unproven
interaction with the dispatcher's POSIX worker signal policy. `compio-term`
uses `SIGWINCH` plus a signal-hook self-pipe for resize, while
`compio-dispatcher` 0.11 records that it
blocks standard POSIX signals on worker threads
([Unix terminal source](../../crates/compio-term/src/sys/unix.rs),
[dispatcher changelog](../../../../Org/Compio/compio/compio-dispatcher/CHANGELOG.md)).

The first prerequisite is dependency alignment. The actor checkout depends on
Git `compio-dispatcher` at Compio commit `c87c3207`, while Intuigram resolves
`compio` 0.19.1 and `compio-runtime` 0.12.3 from crates.io
([actor manifest](../../../../Org/Compio/compio-actor/Cargo.toml),
[actor lockfile](../../../../Org/Compio/compio-actor/Cargo.lock),
[Intuigram lockfile](../../Cargo.lock)). A local probe that called Intuigram's
crates.io `compio::runtime::Runtime::try_current()` from an actor's
`pre_start` returned `false`: the Git runtime's thread-local “current runtime”
is a different Rust package instance. A Telegram client compiled against the
crates.io runtime therefore cannot safely submit its I/O from the actor's Git
runtime worker. Change `compio-actor` to the published crates.io
`compio-dispatcher` 0.11 line, or unify the entire workspace on one Compio
source, before any integration prototype. The compatible
[`compio-dispatcher` 0.11.0 release](https://docs.rs/crate/compio-dispatcher/0.11.0)
is already published, and `compio` 0.19.1 declares that release for its
dispatcher feature
([published `compio` manifest](https://docs.rs/crate/compio/0.19.1/source/Cargo.toml)).

## What `compio-actor` actually provides

The checkout is a small typed actor framework over `compio-dispatcher` and
Flume. `Cluster::spawn` sends a `Send` factory and `Send` startup arguments to a
dispatcher worker; the actor, its state, and its handler futures are then local
to that worker and need not be `Send`. Messages and errors that cross the
cluster boundary must be `Send + 'static`. `Mailbox<A>` exposes typed casts,
typed calls use a one-shot reply, and `Broker<M>` erases the actor type while
retaining one message capability
([actor traits](../../../../Org/Compio/compio-actor/src/actor/mod.rs),
[spawn implementation](../../../../Org/Compio/compio-actor/src/cluster/spawn.rs),
[call implementation](../../../../Org/Compio/compio-actor/src/mailbox/call.rs)).

A compile-time probe confirmed that Intuigram's existing `Effect`,
`AdapterEvent`, and `View` types implement `Send`, so the application-owned
protocol can cross this boundary without leaking Telegram TL or terminal types
([effects](../../crates/intuigram-app/src/protocol/effects.rs),
[adapter events](../../crates/intuigram-app/src/protocol/input.rs),
[view](../../crates/intuigram-app/src/protocol/view.rs)). The production seam
should nevertheless be an Intuigram-owned `TelegramSession` command/event port
that hides `Mailbox`, `Broker`, actor lifecycle hooks, and framework errors;
this keeps `compio-actor` replaceable while its API is experimental.

Each actor has one bounded FIFO message mailbox. Capacity defaults to 64 and is
configurable at spawn. `send` is deliberately non-waiting: it uses `try_send`
and returns the original message as either `Full` or `Closed`; the framework
does not provide an asynchronous capacity wait, coalescing, priority messages,
or reserved command capacity. Stop has its own capacity-one channel and wins a
biased receive, but it is observed only after the current handler future
returns
([mailbox](../../../../Org/Compio/compio-actor/src/mailbox/mod.rs),
[receiver](../../../../Org/Compio/compio-actor/src/mailbox/receiver.rs),
[delivery errors](../../../../Org/Compio/compio-actor/src/mailbox/error.rs)).

Actor lifecycle coverage is useful but narrow: `pre_start`, `post_start`,
serial message handling, `pre_stop`, and `post_stop`; an `ActorHandle` observes
normal stop or the first actor error. Dropping that handle does not stop the
actor. There is no restart policy, child supervision, request deadline,
request cancellation, worker-affinity selector, or health/progress interface.
Calls report `Full`, `Closed`, or `NoReply`, but dropping a call waiter does not
withdraw the already queued request
([delivery loop](../../../../Org/Compio/compio-actor/src/actor/deliver.rs),
[actor handle](../../../../Org/Compio/compio-actor/src/actor/handle.rs),
[call errors](../../../../Org/Compio/compio-actor/src/mailbox/error.rs)).

The framework is early-stage. Its manifest is version 0.1.0. The local branch
has four commits, its remote `main` points at the initial commit, and the source
being evaluated includes an uncommitted module split. The current checkout has
ten integration tests and one README doctest, all of which passed locally, but
the tests cover API/lifecycle behavior rather than I/O actor shutdown,
saturation graphs, panicking workers, restart, or long-lived driver ownership
([manifest](../../../../Org/Compio/compio-actor/Cargo.toml),
[tests](../../../../Org/Compio/compio-actor/tests/actor.rs)). Treat it as a source
dependency under joint development, not yet as a stable infrastructure
dependency.

## Why the naive two-actor design is insufficient

The current Intuigram loop owns a single `Backend`. It moves that value into one
active effect future and does not start another effect until the future returns;
the remaining effects wait in a bounded 64-entry queue. Fair polling keeps
terminal and live-update sources responsive, but a slow Telegram operation
still delays every later database, media, notification, and Telegram effect
([runtime loop](../../crates/intuigram/src/application/runtime_loop.rs),
[runtime types](../../crates/intuigram/src/application/runtime_types.rs),
[single-writer ADR](../adr/0004-single-writer-state.md)). Replacing the effect
body with `telegram_mailbox.call(...).await` leaves that same future active and
therefore preserves the backend queue bottleneck.

The actor adds a second serial boundary. Its receive loop awaits the complete
`Handler::handle` future before receiving the next mailbox item. A history load
or flood-waiting send therefore blocks later Telegram actor commands even
though it no longer blocks terminal drawing
([actor delivery loop](../../../../Org/Compio/compio-actor/src/actor/deliver.rs),
[Telegram flood-wait loop](../../crates/intuigram-telegram/src/source/connection.rs)).
Increasing either mailbox from 64 only postpones saturation; it does not define
which work may be rejected, replaced, or preserved.

There is also a connection-driver liveness requirement. In live mode,
`intuigram-telegram::Client` holds a cloneable `InvocationHandle`, while
`LiveUpdates` owns the `ConnectionDriver`. The handle, response slots, update
stream, and driver share worker-local `Rc` state, and an invocation completes
only while the driver continues to be polled
([Telegram live split](../../crates/intuigram-telegram/src/source/client_connection.rs),
[MTProto driver](../../crates/compio-mtproto/src/driver/mod.rs)). If a Telegram
actor handler awaits `client.history()` while the same actor loop is also
supposed to poll `LiveUpdates`, the invocation deadlocks. The actor must start a
separate local Compio task that owns and continuously polls `LiveUpdates`; that
task and the actor's `Client` handle must stay on the same worker. Compio permits
non-`Send` worker-local tasks, but explicitly does not guarantee spawned tasks
run to completion, so their handles and shutdown must be owned
([Compio runtime](../../../../Org/Compio/compio/compio-runtime/src/lib.rs)).

## Recommended topology

```text
main OS thread / existing Compio runtime
├── TerminalUi + the sole TerminalEvents reader
├── intuigram-app single-writer reducer
├── effect router with bounded, class-specific in-flight work
├── Telegram output ingress and durable-update coordinator
└── terminal RAII shutdown guard
          │ owned Send commands / results / normalized updates
          ▼
one-worker compio-actor Cluster (explicitly one worker)
└── Telegram actor
    ├── Client / InvocationHandle / peer state
    ├── serial command admission and operation policy
    └── worker-local driver task
        └── LiveUpdates / ConnectionDriver, polled continuously

existing dedicated database OS thread
└── AccountDatabase worker + bounded AccountStore endpoint
```

Use an explicitly one-worker cluster for Telegram rather than
`Cluster::new()`. The dispatcher's default is one worker per available CPU, and
it distributes dispatched actor factories through a shared queue without an
actor-affinity API. A one-worker cluster makes placement and runtime ownership
deterministic while still allowing the actor and its driver task to run
cooperatively on that worker
([dispatcher builder](../../../../Org/Compio/compio/compio-dispatcher/src/lib.rs),
[cluster](../../../../Org/Compio/compio-actor/src/cluster/mod.rs)).

Do not make the terminal an actor in this topology. `TerminalEvents` is
documented and implemented as a persistent, process-single event source, and
`TerminalUi` owns synchronous Ratatui/Crossterm drawing plus raw-mode,
alternate-screen, mouse, keyboard-enhancement, cursor, and restoration state
([terminal events](../../crates/intuigram-tui/src/source/terminal.rs),
[compio-term boundary](../../crates/compio-term/README.md)). Keeping it next to
the reducer also avoids copying every immutable `View` through a mailbox and
avoids inventing a latest-frame mailbox policy that `compio-actor` does not
provide. A terminal actor should be reconsidered only after measurements show
that synchronous draw latency—not Telegram/backend serialization—is the actual
bottleneck.

Keep SQLite on its current dedicated blocking thread. `AccountDatabase`
already owns a bounded synchronous command queue and exposes cloneable
nonblocking `AccountStore` methods whose responses are awaitable. Moving that
worker under a Compio actor would add a mailbox but would not make Rusqlite
asynchronous
([database owner](../../crates/intuigram-store/src/account/database.rs),
[account worker](../../crates/intuigram-store/src/account/worker.rs)). The main
durability coordinator should continue to commit normalized Telegram records
and synchronization cursors before sending their adapter events to the reducer,
as required by the local-first and single-writer decisions
([local-first ADR](../adr/0001-local-first-sync.md),
[single-writer ADR](../adr/0004-single-writer-state.md)).

## Backpressure, ordering, and cancellation contract

The prototype needs explicit policies above the generic actor mailbox:

| Traffic | Required policy |
| --- | --- |
| User sends and destructive operations | Lossless admission or visible rejection; retain stable operation IDs; never infer cancellation from a dropped waiter. |
| Foreground Chat history | One active request per Chat; latest navigation may supersede presentation of an older result, but the RPC result must still be reconciled safely. |
| Background history warmup | Bounded and lower priority than foreground work; pause rather than fill the actor mailbox. |
| Telegram updates and cursors | Lossless and ordered through durable commit before application exposure; backpressure may slow the connection but must not silently drop updates. |
| Progress, presence, and “updating” state | Replace/coalesce to the latest value. |
| Shutdown/control | Separate reserved path; never depend on free ordinary mailbox capacity. |

The generic mailbox guarantees serial handling of accepted messages, but it
does not establish domain order across multiple producer threads or across
separately spawned request tasks. Give every request a correlation ID and, for
selection-sensitive reads, a generation. The main reducer remains the only
state writer and decides whether a completed generation is still presentable.
Telegram's live driver can already correlate multiple in-flight RPC results by
MTProto message ID with a fixed outstanding capacity; initially, however, keep
the higher-level `Client` command actor serial until its mutable peer and
normalization state is deliberately split
([driver request correlation](../../crates/compio-mtproto/src/driver/mod.rs),
[Telegram client state](../../crates/intuigram-telegram/src/source/connection.rs)).

`compio-actor`'s built-in stop is necessary but insufficient for application
shutdown. It lets the current handler finish and then drops queued work. It has
no deadline and cannot interrupt an indefinitely flood-waiting handler. The
Telegram interface therefore needs bounded RPC/flood-wait policy and an
explicit shutdown command that stops new admission, classifies every accepted
operation as completed/retryable/abandoned, closes the connection driver, and
only then calls `Mailbox::stop`
([actor receiver](../../../../Org/Compio/compio-actor/src/mailbox/receiver.rs),
[actor finish path](../../../../Org/Compio/compio-actor/src/actor/deliver.rs)).

Orderly process shutdown should be:

1. stop accepting new user intents while keeping the terminal guard alive;
2. stop new Telegram commands and settle or explicitly abandon accepted work;
3. drain Telegram outputs through durable cursor/data commits;
4. stop the driver task, stop the actor, and await its `ActorHandle`;
5. call `Cluster::join` only after every actor handle has completed;
6. shut down and join the Account database worker;
7. restore the terminal through the outer RAII owner.

Calling `Cluster::join` first is unsafe as an application shutdown protocol:
the dispatcher closes its work sender and joins worker runtimes, while actor
tasks are detached from the dispatcher receive loop. The framework's own
example explicitly stops its actor and awaits the actor handle before joining
the cluster
([dispatcher join](../../../../Org/Compio/compio/compio-dispatcher/src/lib.rs),
[actor example](../../../../Org/Compio/compio-actor/README.md)).

## Migration path

1. **Make the dependency reproducible and runtime-compatible.** Commit the
   current actor refactor, pin a revision, switch it to crates.io
   `compio-dispatcher` 0.11 (or patch all Compio dependencies to one source),
   and add an integration test asserting that an actor sees the same current
   Compio runtime used by Intuigram I/O.
2. **Extract Send command/result DTOs without changing behavior.** Keep
   Telegram TL constructors and Compio handles inside `intuigram-telegram`;
   cross the actor boundary only with Intuigram-owned IDs, requests,
   normalized results, connection state, and explicit errors.
3. **Prototype one dedicated Telegram actor behind a feature flag.** Construct
   login/session connections on its worker; do not attempt to move the current
   non-`Send` live client across threads. Start and retain the worker-local
   `LiveUpdates` driver task before accepting commands.
4. **Replace the monolithic backend scheduler.** Route Telegram operations to
   actor calls, storage operations to `AccountStore`, and media/platform work
   to their existing adapters. Allow a bounded set of correlated operations
   rather than one `Backend`-owning future. Preserve reducer event ordering and
   explicit class-specific backpressure.
5. **Keep the terminal and reducer unchanged.** Aggregate terminal input,
   Telegram outputs, database completions, and animation on the main runtime
   using persistent sources and fair polling.
6. **Add fault and saturation tests before making it default.** Cover a
   ten-second history RPC while typing and sending, full command and output
   channels, continuous updates during an RPC, flood wait, actor failure,
   reconnect, shutdown during each phase, and database backpressure.
7. **Adopt only after latency and correctness gates pass.** A useful initial
   target is bounded key-to-render latency under a deliberately stalled
   Telegram actor, zero lost durable updates, no accepted operation left
   unclassified at shutdown, and no runtime-source mismatch.

## Principal risks

- **False isolation:** actorizing Telegram while retaining one active main
  effect future leaves sends, media, and storage queued behind history.
- **Driver starvation:** polling `LiveUpdates` from a serial handler deadlocks
  the invocation it is waiting for.
- **Runtime split:** Git and crates.io Compio instances have distinct
  thread-local runtime identities even at compatible version numbers.
- **Backpressure cycles:** a driver task waiting to publish an update can stop
  network progress while the main thread waits for an RPC result. Separate or
  reserved result/update paths and continuous main ingress draining are
  required.
- **Shutdown ambiguity:** dropping call futures or joining the cluster does not
  prove a Telegram operation was not sent.
- **Framework churn:** the evaluated API is version 0.1.0, locally ahead of its
  remote, and currently dirty; Intuigram would be co-developing its runtime
  substrate.
- **Terminal regressions:** worker signal masking, Crossterm's process-global
  reader, and asynchronous View delivery add risk without addressing the
  observed backend bottleneck.

The result is a **go for a narrow, feature-flagged Telegram actor experiment
after dependency alignment**, and a **no-go for a terminal actor or a direct
production rewrite today**. The desired architectural endpoint is still one
single-writer application reducer; the actor is an adapter-isolation boundary,
not a second owner of application state.

## Verification performed

- Inspected `compio-actor` at local `HEAD` `8450072` plus its current
  uncommitted module split, and Compio at `c87c3207`.
- Ran `cargo test --target-dir /private/tmp/compio-actor-viability-target` in
  the actor checkout: 10 integration tests and 1 doctest passed.
- Ran the cross-source runtime probe described above; the actor worker did not
  expose Intuigram's crates.io Compio runtime as current.
- No Intuigram or `compio-actor` source was modified by this research.
