# compio-term

> This crate is an experimental prototype. Its API is intentionally unstable.

`compio-term` tests if Compio can wake a terminal event future directly from TTY readability. It does not use the Crossterm `EventStream` helper thread.

On Unix, the implementation uses `compio::runtime::fd::PollFd` for the wait. On Windows, the implementation attaches the console input handle to the Compio IOCP event wait. Both implementations use the public Crossterm zero-timeout `poll` and `read` API for decoding. They drain events that Crossterm already buffered before they submit another readiness wait.

For each pull request, the repository runs the native backend tests on Linux, macOS, and Windows.

Run the interactive probe from a terminal:

```sh
cargo run -p compio-term --example events
```

Current limits:

- Use one event reader for each process. Poll it on its Compio runtime thread.
- Keyboard, mouse, focus, paste, and other TTY byte events use Compio wakeups.
- `SIGWINCH` uses a signal-hook self-pipe as a Compio wake source. Crossterm continues to produce the resize event.
- Windows console key, mouse, focus, paste, and resize records wake the same persistent stream through the small documented Compio IOCP boundary in the crate.
- Crossterm continues to own parsing, raw mode, and output commands.
