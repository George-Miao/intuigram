# compio-term

> Experimental prototype. The API is intentionally unstable.

`compio-term` tests whether Compio can wake a terminal event future directly
from TTY readability without Crossterm's `EventStream` helper thread.

The Unix implementation uses `compio::runtime::fd::PollFd` for the actual
wait. The Windows implementation attaches the console input handle to
Compio's IOCP event wait. Both delegate decoding to Crossterm's public
zero-timeout `poll`/`read` API and drain events already buffered by Crossterm
before submitting another readiness wait.

Run the interactive probe from a terminal:

```sh
cargo run -p compio-term --example events
```

Current boundaries:

- one event reader per process, polled on its Compio runtime thread;
- keyboard, mouse, focus, paste, and other TTY byte events use Compio wakeups;
- `SIGWINCH` uses a signal-hook self-pipe as a Compio wake source while
  Crossterm remains responsible for producing the resize event;
- Windows console key, mouse, focus, paste, and resize records wake the same
  persistent stream through the crate's small, documented Compio IOCP boundary;
- Crossterm still owns parsing, raw mode, and output commands;
