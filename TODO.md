# Roadmap

## Priorities

- **p-core:** Required for the first daily-drivable PoC.
- **p-high:** Required before Intuigram can reasonably replace a primary Telegram client.
- **p-mid:** Valuable follow-up after the first PoC.
- **p-low:** Deferred backlog with no near-term promise.

## TODO

- [ ] **p-core:** Retain synchronized Chat metadata and Message text until the user explicitly clears Account data or logs out. Keep media and thumbnails in a size-bounded LRU Media Cache with a configurable 2 GiB default; expose usage and precise clear-media/clear-account actions without silently deleting Message text.
- [ ] **p-core:** Add polished onboarding, packaging, migration, and support for public distribution, including a deliberate Telegram application-credential policy.
- [ ] **p-core:** Add responsive TUI layouts for narrow, normal, and wide terminals, including stacked hierarchy navigation and preservation of the Active Chat, Active Message, Message Selection, anchored history, Draft, and interaction target across resize.
- [ ] **p-core:** Restore the last Active Folder and Active Chat when an Account is reopened. Validate both against current synchronized state; if either no longer exists, clear the Active Chat and return to the default Folder.
- [ ] **p-core:** Add click support for activating Chats and Messages, switching Folders, and focusing the Composer without replacing the keyboard hierarchy or terminal-native text selection.
- [ ] **p-core:** Implement the Windows console-input backend for `compio-term` behind its existing `EventStream(sys::EventStream)` boundary, including key, mouse, focus, paste, and resize events without timer polling or a helper thread.
- [ ] **p-core:** Add `Save As` and destination selection for media and file downloads; never overwrite an existing path silently.
- [ ] **p-core:** Add an optional Local Lock that encrypts both the Account cache and Telegram authorization material, unlocked through an OS keyring or passphrase.
- [ ] **p-core:** Add and switch between multiple Accounts while restoring each Account's Folder, Active Chat, Drafts, scroll positions, and notification identity.
- [ ] **p-core:** Add explicit Account exit workflows. `Logout` must revoke the Telegram authorization before deleting local Account data; when offline, do not report success. `Remove locally` deletes the session, Local Records, and Media Cache with an exact warning that the server-side authorization may remain active. Show the Account identity and deletion scope in confirmation.
- [ ] **p-core:** Create, rename, reorder, share, and delete Folders, including editing Telegram inclusion and exclusion rules.
- [ ] **p-core:** Record and send voice and video notes; browse and send stickers, GIFs, and custom emoji; and share contacts.
- [ ] **p-core:** Manage Scheduled Messages per Chat: schedule for an explicitly zoned local date and time, send when online where Telegram permits it, and view, edit, reschedule, delete, or send now. Keep scheduled state distinct from Drafts and Pending Actions; delivery remains server-side and survives Intuigram exiting.
- [ ] **p-core:** Add SOCKS5, HTTP CONNECT, and MTProxy transports, including secret parsing, proxy authentication, explicit DNS behavior, connection testing, and automatic fallback.

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

- [ ] **p-mid:** Make keyboard bindings configurable through a Figment-based configuration system. Support layered TOML, YAML, JSON, environment, and command-line sources; keep a complete default keymap; and generate the on-screen Action Bar and Help from the active configuration.
- [ ] **p-mid:** Make the terminal palette configurable, keep Everforest Light as a built-in bright theme, and add automatic light/dark terminal-background detection with an explicit configuration override.
- [ ] **p-mid:** Add sender, date-range, and media-type filters to Chat Search and Global Search.
- [ ] **p-mid:** Add a dedicated Global Search shortcut. Use `Ctrl+Shift+F` when enhanced keyboard input can distinguish it from `Ctrl+F`, fall back to `Ctrl+K` on legacy terminals, and show only the effective binding in the Action Bar and Help.
- [ ] **p-mid:** Add Telegram call support. Until then, calls are explicitly outside the Daily Driver product promise.
- [ ] **p-mid:** Obtain current location from platform services or a configured provider and broadcast live locations. Never use IP geolocation silently.
- [ ] **p-mid:** Investigate Secret Chat feasibility and security requirements without promising implementation. Any future proposal must cover the complete end-to-end protocol, device-bound key lifecycle, durable encrypted state, sequence validation, self-destruct behavior, interoperability, and independent security review.
