# Roadmap

## Priorities

- **p-core:** This work is necessary for the first daily-drivable PoC.
- **p-high:** This work is necessary before Intuigram can replace a primary Telegram client.
- **p-mid:** This work is useful after the first PoC.
- **p-low:** This work is in the deferred backlog. It has no near-term promise.

## TODO

- [ ] **p-high:** Make keyboard bindings configurable through a Figment-based configuration system. Support layered TOML, YAML, JSON, environment, and command-line sources. Keep a complete default keymap. Generate the Action Bar and Help from the active configuration.
- [ ] **p-high:** Make the terminal palette configurable. Keep Everforest Light as a built-in bright theme. Add automatic detection of a light or dark terminal background. Provide an explicit configuration override.
- [ ] **p-high:** Add sender, date-range, and media-type filters to Chat Search and Global Search.
- [ ] **p-high:** Measure image decoding, resizing, Unicode avatar fallback tiles, and terminal-protocol encoding in image-heavy Transcripts. Move measured blocking CPU work to `spawn_blocking` or the dedicated graphics worker. Preserve ordered placements and responsive input.
- [ ] **p-high:** Add a dedicated Global Search shortcut. Use `Ctrl+Shift+F` when enhanced keyboard input can distinguish it from `Ctrl+F`. Use `Ctrl+K` on legacy terminals. Show only the effective binding in the Action Bar and Help.
- [ ] **p-high:** Add Telegram call support. Until this work is complete, calls stay outside the Daily Driver product promise.
- [ ] **p-high:** Get the current location from platform services or a configured provider. Broadcast live locations. Never use IP geolocation without an explicit user action.
- [ ] **p-high:** Investigate Secret Chat feasibility and security requirements without an implementation promise. Each future proposal must include the complete end-to-end protocol, device-bound key lifecycle, durable encrypted state, sequence validation, self-destruct behavior, interoperability, and an independent security review.
- [ ] **p-high:** Keep one blank terminal row between the Folder strip and the Action Bar. Remove the extra vertical gap without changing either region's content or behavior.
- [ ] **p-high:** Add playback for Telegram audio files and voice messages. Provide play, pause, seek, progress, and stop controls. Keep terminal input and rendering responsive during playback.
