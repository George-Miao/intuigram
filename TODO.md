# Roadmap

## Priorities

- **p-core:** Required for the first daily-drivable PoC.
- **p-high:** Required before Intuigram can reasonably replace a primary Telegram client.
- **p-mid:** Valuable follow-up after the first PoC.
- **p-low:** Deferred backlog with no near-term promise.

## TODO

- [ ] **p-core:** Add Message Selection as state distinct from the Active Message. Support toggling one or more Messages, visibly mark the selected set, route compatible batch actions through it, preserve valid selections across redraw and resize, and clear stale selections when their Chat or history disappears.
- [ ] **p-core:** Add Account management inside the application hierarchy. List and switch Accounts without restarting, start Add Account from the TUI, and expose safe Logout and Remove Locally confirmations while keeping each Account's state isolated.
- [ ] **p-core:** Add Folder lifecycle management inside the TUI and `intuigram-app`: create, rename, reorder, share, delete, and edit inclusion/exclusion rules, with Telegram results reconciled into the visible Folder strip.
- [ ] **p-core:** Add rich-media sending inside the Composer flow: browse Telegram GIF/sticker libraries, send files as the chosen media kind, record voice/video where supported, and send contacts, while representing progress and failure through typed state-owner events.
- [ ] **p-core:** Add Scheduled Message management inside an Active Chat: create for a UTC-offset time or when online, list, edit, reschedule, delete, and send now, while keeping scheduled history distinct from ordinary Message History.

- [ ] **p-high:** Support mouse scrolling in the Transcript, Composer cursor positioning, and visible-action invocation after click selection is stable. Preserve access to terminal-native text selection.
- [ ] **p-high:** Add first-class forum Topics. Opening a forum Chat shows its Topic list; each Topic keeps independent unread state, Draft, pin state, and position; General remains visible; and Channel replies open in Details or the narrow-screen navigation stack.
- [ ] **p-high:** Add topic-enabled bot Private Chats.
- [ ] **p-high:** Add Saved Messages 2.0 per-origin dialogs.
- [ ] **p-high:** Add Channel direct messages backed by monoforum dialogs.
- [ ] **p-high:** Add specialized interactive rendering for live locations, games, invoices, paid media, giveaways, gifts, shared Stories, and TODO lists.
- [ ] **p-high:** Send static locations through coordinates, pasted map links, or place search.
- [ ] **p-high:** Integrate a configurable external path picker for attaching local files. Keep the picker boundary reusable by the later download-destination workflow.
- [ ] **p-high:** Add a durable Outbox that survives restart, preserves operation ordering and referenced media, and exposes safe retry, cancellation, expiry, and conflict handling.
- [ ] **p-high:** Allow selected Chats to keep media offline outside ordinary Media Cache eviction.
- [ ] **p-high:** Investigate reusing Yazi's image-preview adapter or its component libraries. Cover Kitty Unicode placeholders and legacy graphics, iTerm2 inline images, Sixel, X11/Wayland through Überzug++, and Chafa as a text fallback; document terminal detection, tmux/Zellij passthrough, Alacritty behavior, licensing, and the smallest maintainable integration seam before choosing an implementation.

- [ ] **p-mid:** Make keyboard bindings configurable through a Figment-based configuration system. Support layered TOML, YAML, JSON, environment, and command-line sources; keep a complete default keymap; and generate the on-screen Action Bar and Help from the active configuration.
- [ ] **p-mid:** Make the terminal palette configurable, keep Everforest Light as a built-in bright theme, and add automatic light/dark terminal-background detection with an explicit configuration override.
- [ ] **p-mid:** Add sender, date-range, and media-type filters to Chat Search and Global Search.
- [ ] **p-mid:** Add a dedicated Global Search shortcut. Use `Ctrl+Shift+F` when enhanced keyboard input can distinguish it from `Ctrl+F`, fall back to `Ctrl+K` on legacy terminals, and show only the effective binding in the Action Bar and Help.
- [ ] **p-mid:** Add Telegram call support. Until then, calls are explicitly outside the Daily Driver product promise.
- [ ] **p-mid:** Obtain current location from platform services or a configured provider and broadcast live locations. Never use IP geolocation silently.
- [ ] **p-mid:** Investigate Secret Chat feasibility and security requirements without promising implementation. Any future proposal must cover the complete end-to-end protocol, device-bound key lifecycle, durable encrypted state, sequence validation, self-destruct behavior, interoperability, and independent security review.
