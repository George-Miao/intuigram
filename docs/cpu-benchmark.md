# CPU comparison

Use the same machine, power mode, display, Account, Folder, Chat, terminal size, and sample duration for both clients. Build Intuigram in release mode, start it with the normal Account data, let both clients finish synchronization, and then find the two process IDs:

```sh
cargo build --release -p intuigram
pgrep -fl '/intuigram$|Telegram.app/Contents/MacOS/Telegram'
```

Measure two 30-second phases independently. For the idle phase, leave the same Chat visible and provide no input. For rapid navigation, repeatedly traverse the same ten Chats at roughly the same rate in each client. Run the sampler during each phase:

```sh
scripts/compare-cpu.zsh <intuigram-pid> <telegram-swift-pid> 30
```

The sampler observes both processes simultaneously and fails unless Intuigram's mean CPU stays below Telegram Swift's mean. Record the two command outputs with the tested commit when reporting a regression. The hermetic application test `idle_runtime_parks_until_a_registered_source_wakes` separately guards against redundant idle wakeups, while `ignored_terminal_input_does_not_redraw_an_unchanged_view` guards against unnecessary full redraws.
