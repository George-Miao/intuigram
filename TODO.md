# Roadmap

## Priorities

- **p-core:** Required for the first daily-drivable PoC.
- **p-high:** Required before Intuigram can reasonably replace a primary Telegram client.
- **p-mid:** Valuable follow-up after the first PoC.
- **p-low:** Deferred backlog with no near-term promise.

## TODO

- [ ] **p-high:** Make keyboard bindings configurable through a Figment-based configuration system. Support layered TOML, YAML, JSON, environment, and command-line sources; keep a complete default keymap; and generate the on-screen Action Bar and Help from the active configuration.
- [ ] **p-high:** Make the terminal palette configurable, keep Everforest Light as a built-in bright theme, and add automatic light/dark terminal-background detection with an explicit configuration override.
- [ ] **p-high:** Add sender, date-range, and media-type filters to Chat Search and Global Search.
- [ ] **p-high:** Profile image decoding, resizing, text fallback, and terminal-protocol encoding under image-heavy Transcripts; move any measured blocking CPU work to `spawn_blocking` or the dedicated graphics worker while preserving ordered placements and responsive input.
- [ ] **p-high:** Add a dedicated Global Search shortcut. Use `Ctrl+Shift+F` when enhanced keyboard input can distinguish it from `Ctrl+F`, fall back to `Ctrl+K` on legacy terminals, and show only the effective binding in the Action Bar and Help.
- [ ] **p-high:** Add Telegram call support. Until then, calls are explicitly outside the Daily Driver product promise.
- [ ] **p-high:** Obtain current location from platform services or a configured provider and broadcast live locations. Never use IP geolocation silently.
- [ ] **p-high:** Investigate Secret Chat feasibility and security requirements without promising implementation. Any future proposal must cover the complete end-to-end protocol, device-bound key lifecycle, durable encrypted state, sequence validation, self-destruct behavior, interoperability, and independent security review.
