# Roadmap

## Priorities

- **p-core:** Required for the first daily-drivable PoC.
- **p-high:** Required before Intuigram can reasonably replace a primary Telegram client.
- **p-mid:** Valuable follow-up after the first PoC.
- **p-low:** Deferred backlog with no near-term promise.

## TODO

- [x] **p-core:** Collapse the bottom chrome into one context-sensitive line. Omit the Account username and interaction-target labels such as `Chats` or `Composer`; show `connected` only while idle, replace it with the current effort status while work is pending, and place the effective shortcuts after that status on the same line.
- [x] **p-core:** Reduce the Chat list's left content padding by one terminal cell without weakening its focus or selection affordance.
- [x] **p-core:** Replace separate Active Message action shortcuts with one effective key that opens a selectable Message Actions popup containing every action currently valid for the Active Message or Message Selection.
- [x] **p-core:** Replace separate Composer-side creation shortcuts with one effective key that opens a selectable Composer Actions popup for media, polls, and the other currently valid creation flows.
- [x] **p-core:** Give every popup one terminal cell of internal padding while keeping pointer semantics and narrow-terminal clipping correct.
- [x] **p-core:** Widen the side-by-side Chat list column slightly while preserving adaptive layout behavior and a usable Transcript width.
- [x] **p-core:** Replace the subtle fresh-Chat loading indicator with an obvious polished ASCII animation themed around Intuigram and Telegram. Keep cached Transcript content visible during partial updates and respect reduced terminal space.
- [x] **p-core:** Keep the Active Chat near 70% of the visible Chat list while navigating downward and near 30% while navigating upward, scrolling only after the corresponding directional cap is reached.
- [x] **p-core:** Show Message editing context as a one-line quoted preview above the Composer with one terminal cell of padding. Remove the left-side `Editing <id>` label while preserving a clear edit affordance and the existing cancel/save behavior.
- [x] **p-core:** Extract terminal image rendering into a reusable standalone `rasterm` crate with a small interface free of Ratatui and Intuigram model types. Cover terminal detection, image lifecycle, tmux/Zellij passthrough, and hermetic protocol tests; investigate Yazi's adapter and component libraries; and support Kitty graphics across compatible terminals, legacy Kitty placement, iTerm2 inline images, Sixel, X11/Wayland through Überzug++, and Chafa text fallback with documented licensing and Alacritty behavior.

- [ ] **p-high:** Replace line-oriented first-run configuration and login prompts with a centered, stepped TUI flow for API ID, API hash, phone number, login code, and 2FA password. Mask secrets, show validation and recoverable errors in place, and preserve completed non-secret steps when moving backward or retrying.
- [ ] **p-high:** Support mouse Composer cursor positioning and visible-action invocation now that click selection and pane scrolling are stable. Preserve access to terminal-native text selection.
- [ ] **p-high:** Add first-class forum Topics. Opening a forum Chat shows its Topic list; each Topic keeps independent unread state, Draft, pin state, and position; General remains visible; and Channel replies open in Details or the narrow-screen navigation stack.
- [ ] **p-high:** Add topic-enabled bot Private Chats.
- [ ] **p-high:** Add Saved Messages 2.0 per-origin dialogs.
- [ ] **p-high:** Add Channel direct messages backed by monoforum dialogs.
- [ ] **p-high:** Add specialized interactive rendering for live locations, games, invoices, paid media, giveaways, gifts, shared Stories, and TODO lists.
- [ ] **p-high:** Send static locations through coordinates, pasted map links, or place search.
- [ ] **p-high:** Integrate a configurable external path picker for attaching local files. Keep the picker boundary reusable by the later download-destination workflow.
- [ ] **p-high:** Add a durable Outbox that survives restart, preserves operation ordering and referenced media, and exposes safe retry, cancellation, expiry, and conflict handling.
- [ ] **p-high:** Allow selected Chats to keep media offline outside ordinary Media Cache eviction.
- [ ] **p-high:** Append `...` when a Chat name &amp; message preview is capped to fit its available width. Perform terminal-cell-aware truncation and keep the ellipsis inside the allocated Chat-list width. Message count should always be visible on the rightmost side of chat name. 

- [ ] **p-mid:** Make keyboard bindings configurable through a Figment-based configuration system. Support layered TOML, YAML, JSON, environment, and command-line sources; keep a complete default keymap; and generate the on-screen Action Bar and Help from the active configuration.
- [ ] **p-mid:** Make the terminal palette configurable, keep Everforest Light as a built-in bright theme, and add automatic light/dark terminal-background detection with an explicit configuration override.
- [ ] **p-mid:** Add sender, date-range, and media-type filters to Chat Search and Global Search.
- [ ] **p-mid:** Add a dedicated Global Search shortcut. Use `Ctrl+Shift+F` when enhanced keyboard input can distinguish it from `Ctrl+F`, fall back to `Ctrl+K` on legacy terminals, and show only the effective binding in the Action Bar and Help.
- [ ] **p-mid:** Add Telegram call support. Until then, calls are explicitly outside the Daily Driver product promise.
- [ ] **p-mid:** Obtain current location from platform services or a configured provider and broadcast live locations. Never use IP geolocation silently.
- [ ] **p-mid:** Investigate Secret Chat feasibility and security requirements without promising implementation. Any future proposal must cover the complete end-to-end protocol, device-bound key lifecycle, durable encrypted state, sequence validation, self-destruct behavior, interoperability, and independent security review.
