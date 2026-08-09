# Roadmap

## Priorities

- **p-core:** Required for the first daily-drivable PoC.
- **p-high:** Required before Intuigram can reasonably replace a primary Telegram client.
- **p-mid:** Valuable follow-up after the first PoC.
- **p-low:** Deferred backlog with no near-term promise.

## TODO

- [ ] **p-core:** Replace line-oriented first-run configuration and login prompts with a centered, stepped TUI flow for API ID, API hash, phone number, login code, and 2FA password. Mask secrets, show validation and recoverable errors in place, and preserve completed non-secret steps when moving backward or retrying.
- [ ] **p-core:** Support mouse Composer cursor positioning and visible-action invocation now that click selection and pane scrolling are stable. Preserve access to terminal-native text selection.
- [ ] **p-core:** Add first-class forum Topics. Opening a forum Chat shows its Topic list; each Topic keeps independent unread state, Draft, pin state, and position; General remains visible; and Channel replies open in Details or the narrow-screen navigation stack.
- [ ] **p-core:** Add topic-enabled bot Private Chats.
- [ ] **p-core:** Add Saved Messages 2.0 per-origin dialogs.
- [ ] **p-core:** Add Channel direct messages backed by monoforum dialogs.
- [ ] **p-core:** Add specialized interactive rendering for live locations, games, invoices, paid media, giveaways, gifts, shared Stories, and TODO lists.
- [ ] **p-core:** Send static locations through coordinates, pasted map links, or place search.
- [ ] **p-core:** Integrate a configurable external path picker for attaching local files. Keep the picker boundary reusable by the later download-destination workflow.
- [ ] **p-core:** Add a durable Outbox that survives restart, preserves operation ordering and referenced media, and exposes safe retry, cancellation, expiry, and conflict handling.
- [ ] **p-core:** Allow selected Chats to keep media offline outside ordinary Media Cache eviction.
- [ ] **p-core:** Render sender avatar images in the Transcript and use a two-row `[avatar] [username]` / `[avatar] [message]` layout. Fall back to the current two-character name avatar when an image cannot be rendered.
- [ ] **p-core:** Render group Chat-list rows as `[Chat avatar] [Chat name] [Message time]` above `[sender avatar] [message preview]`, preserving unread count, selection, truncation, and narrow-terminal behavior.
- [ ] **p-high:** Make keyboard bindings configurable through a Figment-based configuration system. Support layered TOML, YAML, JSON, environment, and command-line sources; keep a complete default keymap; and generate the on-screen Action Bar and Help from the active configuration.
- [ ] **p-high:** Make the terminal palette configurable, keep Everforest Light as a built-in bright theme, and add automatic light/dark terminal-background detection with an explicit configuration override.
- [ ] **p-high:** Add sender, date-range, and media-type filters to Chat Search and Global Search.
- [ ] **p-high:** Add a dedicated Global Search shortcut. Use `Ctrl+Shift+F` when enhanced keyboard input can distinguish it from `Ctrl+F`, fall back to `Ctrl+K` on legacy terminals, and show only the effective binding in the Action Bar and Help.
- [ ] **p-high:** Add Telegram call support. Until then, calls are explicitly outside the Daily Driver product promise.
- [ ] **p-high:** Obtain current location from platform services or a configured provider and broadcast live locations. Never use IP geolocation silently.
- [ ] **p-high:** Investigate Secret Chat feasibility and security requirements without promising implementation. Any future proposal must cover the complete end-to-end protocol, device-bound key lifecycle, durable encrypted state, sequence validation, self-destruct behavior, interoperability, and independent security review.
