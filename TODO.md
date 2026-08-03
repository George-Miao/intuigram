# Roadmap

## Priorities

- **p-core:** Required for the first daily-drivable PoC.
- **p-high:** Required before Intuigram can reasonably replace a primary Telegram client.
- **p-mid:** Valuable follow-up after the first PoC.
- **p-low:** Deferred backlog with no near-term promise.

## TODO

- [x] **p-core:** Convert the repository to a virtual Cargo workspace with every existing package under `crates/`, keep dependencies one-way toward `intuigram-app`, and compose adapters only in `intuigram`.
- [x] **p-core:** Consolidate terminal input, application-state reduction, Telegram, and nonblocking adapters onto one Compio runtime thread. Remove `async_channel`, the state-owner thread, the UI thread, the blocking backend loop, and nested `Runtime::block_on` calls. Drive persistent event sources and a bounded set of pending effect futures directly from the composition loop without cancelling and recreating in-flight Compio operations when another source wakes; retain dedicated worker threads for blocking SQLite work.
- [x] **p-core:** Target technical users who provide their own Telegram API ID and hash.
- [x] **p-core:** Authenticate by phone number, login code, and Telegram 2FA password.
- [x] **p-core:** Enforce owner-only permissions for Account storage, exclude credentials and Message bodies from logs, and document that the session and Synchronized Cache are unencrypted at rest.
- [ ] **p-high:** Retain synchronized Chat metadata and Message text until the user explicitly clears Account data or logs out. Keep media and thumbnails in a size-bounded LRU Media Cache with a configurable 2 GiB default; expose usage and precise clear-media/clear-account actions without silently deleting Message text.
- [x] **p-core:** Use `rusqlite` and Refinery for transactional, versioned database migrations with pre-migration backups. Store cross-Account runtime records in `global.db`, each Account's durable state in `<telegram-user-id>.db`, and incomplete login state in `.pending.db`; close and atomically rename the pending database without replacing an existing Account database after authorization. Run integrity and foreign-key checks after migration, preserve original files on failure, and enforce owner-only permissions.
- [ ] **p-core:** Enter read-only recovery for a corrupt or unmigratable Account database, preserve the original, show exact database and backup paths, and offer Retry, open-backup-location, and explicit Rebuild Cache actions. Rebuild Cache must preserve authorization, Drafts, Draft History, configuration, and every unique Local Record; otherwise require export or explicit abandonment.
- [ ] **p-core:** Build raw Telegram integration on `compio-mtproto` and current generated TL types instead of `grammers-client`, `grammers-mtsender`, or `grammers-session`. Initially retain mature low-level grammers TL generation, MTProto codec, and cryptographic crates; own Compio transport, request/session machinery, update reconciliation, and complete Intuigram-domain translation with protocol conformance tests.
- [x] **p-core:** Drive a persistent single-owner MTProto connection, expose an awaitable update stream, and continuously receive Telegram updates while otherwise idle so the Active Chat and Chat list update without a history reload.
- [x] **p-core:** Add `intuigram-media` and `rich-clipboard` only after their planned behavior demonstrates meaningful crate seams.
- [ ] **p-core:** Support direct Telegram TCP connections through `compio-mtproto`, while designing its transport seam for later proxy adapters. Use grammers' sender implementation as a behavioral reference for MTProto state transitions, retries, and edge cases without preserving its Tokio-specific interface.
- [x] **p-core:** Support root cloud Chats: human and ordinary bot Private Chats, Saved Messages, Basic Groups, Supergroups, Gigagroups, Channels, and explicit inaccessible-peer presentation.
- [ ] **p-core:** Support ordinary Message Threads and Channel comments. `Ctrl+R` replies to the Active Message; `Ctrl+T` opens its Thread in Details or stacked navigation; Thread read state, Draft, anchored history, and live updates remain independent; returning restores the parent Transcript.
- [ ] **p-core:** Synchronize existing Folder order and membership, switch from the bottom Folder strip, show Folder unread counts, access Archive, and add or remove the Active Chat.
- [ ] **p-core:** Render rich text; replies, forwards, reactions, edits, pins, delivery and read markers, and counters; photos, albums, videos, animations, stickers, custom emoji, files, audio, voice and video notes; link previews; interactive polls and quizzes; contacts, locations, venues, and dice results; and clear service events.
- [ ] **p-core:** Present specialized or unknown Message content as an informative Media Card or Unsupported Content instead of omitting it or failing synchronization.
- [ ] **p-core:** Send multiline rich text, replies, forwards, edits, deletions, reactions, photos, videos, files, link previews, and polls. Support a smart Clipboard Paste action in every open Chat: query the native clipboard through platform adapters, insert text into the Draft, turn images into photo attachment candidates, and turn copied files into file attachment candidates. Show unsupported formats or unavailable clipboard integration clearly, and never interpret pasted content as shell commands.
- [ ] **p-core:** Accept Telegram operations optimistically while disconnected, show their Pending Action state, and retry automatically after reconnection while Intuigram remains running. Preserve failed text as a Draft and expose terminal failure clearly; do not promise that pending operations survive exit or a crash.
- [ ] **p-core:** Normalize Telegram updates into Intuigram-owned records and atomically commit each durable state change with its synchronization cursor before exposing it to the TUI. Persist Draft changes before reporting them saved; represent optimistic durability and network acknowledgement separately.
- [ ] **p-core:** Open ordinary web links through the OS browser and handle supported Telegram links internally. Reveal and confirm disguised, mismatched, or suspicious destinations before opening. Download media and files with collision-safe names to the configured download directory, defaulting to the platform's Downloads directory, without prompting for a path. Open non-launchable downloads through the OS-associated application; for executables, scripts, desktop entries, and other launchable content, offer only reveal-in-folder with a warning.
- [ ] **p-high:** Add polished onboarding, packaging, migration, and support for public distribution, including a deliberate Telegram application-credential policy.
- [ ] **p-high:** Add responsive TUI layouts for narrow, normal, and wide terminals, including stacked hierarchy navigation and preservation of the Active Chat, Active Message, Message Selection, anchored history, Draft, and interaction target across resize.
- [ ] **p-high:** Support mouse input for activating Chats and Messages, switching Folders, scrolling the Transcript, positioning the Composer cursor, and invoking visible actions without replacing the keyboard hierarchy. Preserve access to terminal-native text selection.
- [ ] **p-high:** Implement the Windows console-input backend for `compio-term` behind its existing `EventStream(sys::EventStream)` boundary, including key, mouse, focus, paste, and resize events without timer polling or a helper thread.
- [x] **p-high:** Add QR login rendered directly in the terminal, including token expiry, data-center migration, and 2FA continuation.
- [ ] **p-high:** Add `Save As` and destination selection for media and file downloads; never overwrite an existing path silently.
- [ ] **p-high:** Add an optional Local Lock that encrypts both the Account cache and Telegram authorization material, unlocked through an OS keyring or passphrase.
- [ ] **p-high:** Add and switch between multiple Accounts while restoring each Account's Folder, focused Chat, Drafts, scroll positions, and notification identity.
- [ ] **p-high:** Add explicit Account exit workflows. `Logout` must revoke the Telegram authorization before deleting local Account data; when offline, do not report success. `Remove locally` deletes the session, Local Records, and Media Cache with an exact warning that the server-side authorization may remain active. Show the Account identity and deletion scope in confirmation.
- [ ] **p-high:** Create, rename, reorder, share, and delete Folders, including editing Telegram inclusion and exclusion rules.
- [ ] **p-high:** Record and send voice and video notes; browse and send stickers, GIFs, and custom emoji; and share contacts.
- [ ] **p-high:** Manage Scheduled Messages per Chat: schedule for an explicitly zoned local date and time, send when online where Telegram permits it, and view, edit, reschedule, delete, or send now. Keep scheduled state distinct from Drafts and Pending Actions; delivery remains server-side and survives Intuigram exiting.
- [ ] **p-high:** Add SOCKS5, HTTP CONNECT, and MTProxy transports, including secret parsing, proxy authentication, explicit DNS behavior, connection testing, and automatic fallback.
- [ ] **p-low:** Make keyboard bindings configurable through a Figment-based configuration system. Support layered TOML, YAML, JSON, environment, and command-line sources; keep a complete default keymap; and generate the on-screen Action Bar and Help from the active configuration.
- [ ] **p-low:** Make the terminal palette configurable, keep Everforest Light as a built-in bright theme, and add automatic light/dark terminal-background detection with an explicit configuration override.
- [ ] **p-mid:** Add first-class forum Topics. Opening a forum Chat shows its Topic list; each Topic keeps independent unread state, Draft, pin state, and position; General remains visible; and channel replies open in Details or the narrow-screen navigation stack.
- [ ] **p-mid:** Add topic-enabled bot Private Chats.
- [ ] **p-mid:** Add Saved Messages 2.0 per-origin dialogs.
- [ ] **p-mid:** Add Channel direct messages backed by monoforum dialogs.
- [ ] **p-mid:** Add specialized interactive rendering for live locations, games, invoices, paid media, giveaways, gifts, shared Stories, and TODO lists.
- [ ] **p-mid:** Send static locations through coordinates, pasted map links, or place search.
- [ ] **p-mid:** Integrate a configurable external path picker for attaching local files. Keep the picker boundary reusable by the later download-destination workflow.
- [ ] **p-mid:** Add a durable Outbox that survives restart, preserves operation ordering and referenced media, and exposes safe retry, cancellation, expiry, and conflict handling.
- [ ] **p-mid:** Allow selected Chats to keep media offline outside ordinary Media Cache eviction.
- [ ] **p-low:** Add sender, date-range, and media-type filters to Chat Search and Global Search.
- [ ] **p-low:** Add a dedicated Global Search shortcut. Use `Ctrl+Shift+F` when enhanced keyboard input can distinguish it from `Ctrl+F`, fall back to `Ctrl+K` on legacy terminals, and show only the effective binding in the Action Bar and Help.
- [ ] **p-high:** Add `Ctrl+O` as the default attachment-selection shortcut, generated through the configurable keymap and shown in the Action Bar when available.
- [ ] **p-low:** Add Telegram call support. Until then, calls are explicitly outside the Daily Driver product promise.
- [ ] **p-low:** Obtain current location from platform services or a configured provider and broadcast live locations. Never use IP geolocation silently.
- [ ] **p-low:** Investigate Secret Chat feasibility and security requirements without promising implementation. Any future proposal must cover the complete end-to-end protocol, device-bound key lifecycle, durable encrypted state, sequence validation, self-destruct behavior, interoperability, and independent security review.
