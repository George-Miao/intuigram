# Single-threaded Compio UI orchestration

Research snapshot: 2026-08-02

## Question

Can Popgram represent the TUI as a future and run it cooperatively with the
application-state owner and Telegram I/O on one Compio 0.19.1 runtime thread,
in the style of Winio?

## Conclusion

Yes. This is a good fit for Popgram, with one important qualification: putting
several futures on one runtime only creates concurrency when every future
returns `Poll::Pending` promptly instead of blocking the thread. The TUI can be
a long-lived future, but Crossterm input must be polled without waiting and
Ratatui drawing remains a short synchronous section. SQLite should keep its
dedicated worker thread.

Winio is a sound analogy, not an implementation to copy. Winio describes itself
as a single-threaded asynchronous GUI runtime compatible with Compio, where I/O
can be issued on the GUI thread without blocking the interface. Its example
passes a root component future to `App::block_on`, and its component's event
listener is an async function
([Winio README](https://github.com/compio-rs/winio#readme),
[Winio crate docs](https://docs.rs/winio/latest/winio/)). Popgram can use the
same broad arrangement while retaining its own typed channels and immutable
view snapshots.

## What Compio provides

Compio 0.19.1 calls `Runtime` thread-local and marks it `!Send` and `!Sync`. Its
`spawn` accepts `F: Future + 'static` without a `Send` bound, and documents that
spawned tasks execute concurrently with other tasks
([Runtime](https://docs.rs/compio/0.19.1/compio/runtime/struct.Runtime.html),
[spawn](https://docs.rs/compio/0.19.1/compio/runtime/fn.spawn.html)). Therefore a
single runtime thread can own the non-`Send` Telegram client and poll independent
UI, state, and protocol tasks cooperatively.

`Runtime::block_on` should drive one orchestration future. That future should
spawn the long-lived actors with `compio::runtime::spawn`; it should not manually
poll all application futures. Compio also supplies async timers (`sleep`,
`interval`, and timeouts), which are enough to wake a terminal-input polling
loop without blocking
([Compio time module](https://docs.rs/compio/0.19.1/compio/runtime/time/)).

`FuturesUnordered` is not a runtime or a task scheduler. It is a stream of
same-typed futures that yields completions in any order, and its caller must
continue polling `poll_next` to activate and receive wakeups
([FuturesUnordered](https://docs.rs/futures-util/0.3.33/futures_util/stream/struct.FuturesUnordered.html)).
More importantly, Compio warns that sub-executors such as `FuturesUnordered`
replace the waker and can lose Compio's single-threaded cancellation/personality
metadata
([Compio `FutureExt`](https://docs.rs/compio/0.19.1/compio/runtime/trait.FutureExt.html)).
Use Compio tasks for the main topology. `FuturesUnordered` is reasonable only
inside a supervisor for homogeneous task exits that do not rely on those
Compio future extensions; it should not drive sockets or the UI.

## Recommended task topology

One OS thread runs one `compio::runtime::Runtime` and owns:

```text
root supervisor future
├── TUI task
│   ├── synchronously render the newest immutable View
│   └── await the Compio terminal stream, View changes, or shutdown
├── popgram-app state-owner task
│   └── select typed Intents and AdapterEvents; emit Views and Effects
├── Telegram connection actor
│   ├── own Client / MTProto connection state
│   ├── accept bounded commands
│   ├── continuously receive protocol traffic and updates
│   └── emit correlated results and normalized events
├── media/network tasks, spawned with explicit concurrency limits
└── shutdown supervisor

dedicated database OS thread
└── own rusqlite Connection; consume commands and return typed results
```

This preserves the project's single-writer state rule while removing the
current need for separate TUI and app threads. A blocked Telegram request then
suspends only the Telegram task, so the runtime can keep polling TUI and state.
That is exactly the benefit `spawn` promises, provided the Telegram future is
actually waiting on Compio I/O rather than executing blocking work.

The database remains an intentional exception. SQLite calls are synchronous,
and `rusqlite::Connection` is `Send` but not `Sync`; keeping one connection on a
dedicated worker gives it one owner without blocking the runtime
([rusqlite `Connection`](https://docs.rs/rusqlite/0.39.0/rusqlite/struct.Connection.html)).
The runtime communicates with that thread through bounded commands and typed
responses.

## Terminal input and rendering

`compio-term::EventStream` now supplies the Unix input future without timer
polling or Crossterm's helper thread. Its `sys::unix` backend first drains
Crossterm's already-decoded event queue, then awaits Compio readiness for the
TTY and a signal-hook self-pipe for `SIGWINCH`. After either wake it lets
Crossterm perform the byte read and event decoding. This preserves Crossterm's
public event model while making an idle input task suspend on the Compio
runtime.

The TUI races that stream against immutable View updates and shutdown. It must
remain the only consumer of Crossterm's global event reader; cursor-position
and capability queries will eventually need routing through the same terminal
session rather than calling `read`, `poll`, or Crossterm's `EventStream`
independently. Windows console input is a p-high backend behind the same public
wrapper.

Representing the UI as a future does not make terminal operations asynchronous.
Ratatui's `Terminal::draw` performs a complete synchronous render pass,
including buffer diff, backend writes, cursor updates, and flush
([Ratatui `Terminal`](https://docs.rs/ratatui/0.30.2/ratatui/struct.Terminal.html)).
Crossterm raw-mode and alternate-screen setup are synchronous too
([Crossterm terminal docs](https://docs.rs/crossterm/0.29.0/crossterm/terminal/)).
Keep setup, teardown, and each render pass small; never perform Telegram,
database, media, clipboard, or expensive normalization work in them. A very
slow terminal backend can still stall the sole thread, so retain the option to
restore a separate TUI thread if measurement shows this matters.

## Backpressure and request concurrency

Bounded channels remain useful, but async sends can form a logical deadlock even
though they do not block the OS thread. `async-channel::send` waits for capacity,
whereas `try_send` reports `Full`
([async-channel `Sender`](https://docs.rs/async-channel/2.5.0/async_channel/struct.Sender.html)).
Avoid a cycle such as:

```text
app awaits space in Telegram-command queue
Telegram awaits space in app-event queue
neither actor drains its inbound queue
```

Give each actor one continuously draining ingress loop. Do not await a bounded
send while withholding a response or resource required by that send's consumer.
Use explicit policies by message class: correlated command results must be
delivered or fail the request; replaceable Views and progress notifications may
coalesce to the latest value; repeated navigation loads may use latest-wins;
shutdown/control traffic needs reserved capacity or a separate path. Saturation
must be observable and tested.

The current Telegram API cannot become concurrent merely by spawning several
call futures. `Client` methods take `&mut self`, and
`EncryptedConnection::invoke` sends one request then reads until that request's
result arrives
([current client](../../crates/popgram-telegram/src/lib.rs),
[current sender](../../crates/compio-mtproto/src/sender.rs)). This correctly
serializes RPCs while still allowing UI responsiveness. True overlapping RPCs
requires the planned connection actor to:

- assign and retain request message IDs;
- queue and batch outbound envelopes;
- run one continuous receive loop;
- correlate each RPC result/error with a responder;
- route independent updates and service acknowledgements;
- preserve retry, ordering, salt, and reconnection semantics.

That actor is a protocol change, not a scheduling trick.

## Cancellation and shutdown risks

Compio explicitly gives no guarantee that a spawned task runs to completion,
and its cancellation metadata should have a clean waker path rather than a
sub-executor
([spawn](https://docs.rs/compio/0.19.1/compio/runtime/fn.spawn.html),
[`FutureExt`](https://docs.rs/compio/0.19.1/compio/runtime/trait.FutureExt.html)).
Popgram should therefore own every long-lived task handle and use explicit
shutdown messages and deadlines. Dropping a task or an in-flight completion I/O
future must not be treated as proof that a Telegram operation was not sent.

Recommended shutdown order:

1. stop accepting new user intents and show shutdown state;
2. ask the app owner to persist the active Draft and stop producing Effects;
3. stop accepting Telegram commands, settle or explicitly abandon each
   correlated request, then close the transport;
4. drain and close the database worker, waiting for committed work;
5. restore terminal state through an outer RAII guard even on task error/panic;
6. await task exits, applying a bounded deadline and reporting incomplete
   shutdown rather than hanging.

CPU-heavy loops must also yield or be chunked. Cooperative scheduling cannot
preempt a future that parses an unbounded update batch or renders indefinitely.

## Prototype before replacing the current composition

1. Add a private single-thread orchestrator behind a feature flag or test-only
   seam; keep public crate interfaces unchanged.
2. Run mock TUI, app-owner, and slow-adapter futures as Compio spawned tasks.
   Use `compio-term::EventStream` in the real TUI and race it against View and
   shutdown channels.
3. Inject a two-second fake history load and assert that key-to-View and
   key-to-render latency remain below a chosen budget (start with 50 ms).
4. Saturate every bounded channel and test its policy: coalesce, reject, reserve,
   or await without a circular dependency.
5. Test shutdown during an idle receive, an RPC, a database transaction, and a
   terminal error; assert terminal restoration and durable Draft behavior.
6. Move the existing sequential Telegram `Client` into the connection task.
   Only after this is stable, prototype request correlation for overlapping RPCs.

If this prototype passes, replace the current OS-thread composition in
`crates/popgram/src/main.rs` and update the single-writer ADR. The likely result
is a simpler and more faithful Compio architecture: one cooperative runtime
thread for UI, app state, and network actors, plus the deliberately blocking
database thread.
