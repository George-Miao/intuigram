# compio-term

> Experimental prototype. The API is intentionally unstable.

`compio-term` tests whether Compio can wake a terminal event future directly
from TTY readability without Crossterm's `EventStream` helper thread.

The current Unix implementation uses `compio::runtime::fd::PollFd` for the
actual wait, then delegates decoding to Crossterm's public zero-timeout
`poll`/`read` API. It drains events already buffered by Crossterm before
submitting another readiness wait.

Run the interactive probe from a terminal:

```sh
cargo run -p compio-term --example events
```

Current boundaries:

- one event reader per process, polled on its Compio runtime thread;
- keyboard, mouse, focus, paste, and other TTY byte events use Compio wakeups;
- `SIGWINCH` uses a signal-hook self-pipe as a Compio wake source while
  Crossterm remains responsible for producing the resize event;
- Crossterm still owns parsing, raw mode, and output commands;
- Windows console input remains a p-high platform backend.
