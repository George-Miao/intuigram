# Intuigram repository guidance

## General rules

- Keep this file correct when the repository structure, toolchain, architecture, or required checks change.
- Use English to communicate with the user.
- Add a comment only when it explains intent or a constraint that is not clear.
- Add tests that verify real behavior or prevent a regression. Do not add placeholder tests.

## Documentation style

- Use ASD-STE100 Simplified Technical English in all Markdown files.
- Use active voice. Name the type or component that does an action. For example, write “`RetryLayer` retries failed operations.”
- Use short, direct sentences in the present tense.
- Start public API documentation with the function of the type or method. Then explain important constraints, defaults, and behavior.
- Describe API semantics precisely. Verify option types, capability requirements, error behavior, overwrite behavior, and version behavior against the implementation.
- Use one term for one concept. Use the terms in the codebase.
- Use parallel grammar in lists. Use punctuation consistently for complete sentences.

## Project direction

Intuigram is a local-first Telegram terminal client. It is designed to replace a primary GUI client for routine communication. The current implementation is a Rust virtual workspace. It uses a single-writer application reducer, a Compio orchestration loop, dedicated Telegram actors for each Account, separate SQLite storage for each Account, optional SQLCipher Local Lock, a dense Ratatui TUI, and a hierarchical process CLI. Calls and Secret Chats are outside the current Daily Driver promise.

At the end of a task, update `TODO.md` if the task is in that file. If the task is not in that file, do not change `TODO.md`.

## Required reading

Before architecture or product work, read:

1. `CONTEXT.md` for the project vocabulary and the specified behavior.
2. `TODO.md` for scope and the `p-core`, `p-high`, `p-mid`, and `p-low` priorities.
3. Each applicable decision in `docs/adr/`.

These documents are the current source of truth. They have priority over `README.md`. When a decision changes, update all affected documents in the same change. Do not put implementation details in `CONTEXT.md`. Add an ADR only for an important architecture decision that has a real tradeoff.

## Target workspace

The target is a virtual Cargo workspace. Each package is under `crates/`.

Declare each dependency that a workspace crate uses in the root `[workspace.dependencies]` table. Member manifests must use `workspace = true` for normal, development, build, target-specific, path, and Git dependencies. Keep dependency versions and shared defaults in one place. Add feature selections in a member only when they are specific to that crate.

- `crates/intuigram` owns the executable entrypoint, Compio runtime startup, colorful Clap command hierarchy, and cross-layer behavior tests. It owns process argument parsing. It starts the TUI when the user gives no subcommand. It sends validated launch arguments to `intuigram-app`.
- `crates/intuigram-app` owns application orchestration, adapter composition, synchronization, recovery, and the main Compio runtime loop. Its modules are directly under `src/`. Do not add a redundant `src/application/` wrapper.
- `crates/intuigram-lib` is the only owner of canonical application state, synchronous transitions, user intents, adapter events, effects, and read-only view data.
- `crates/intuigram-tui` owns terminal input, adaptive layout, and rendering.
- `crates/intuigram-telegram` owns Telegram login, synchronization, raw requests, update reconciliation, and conversion to Intuigram-owned data.
- `crates/intuigram-store` owns durable application records, migrations, backups, and recovery.
- `crates/intuigram-media` owns media transfer, cache policy, and media lifecycle.
- `crates/intuigram-config` owns layered Figment configuration.
- `crates/compio-mtproto` is a reusable Compio-based MTProto connection, session, invocation, and update-stream library.
- `crates/compio-term` provides experimental reusable Compio-native terminal event readiness. Keep its API explicitly unstable until cross-platform behavior is complete.
- `crates/rasterm` owns reusable terminal raster-image detection, cell geometry, protocol encoding, external-renderer commands, and image lifecycle. Keep it independent of Ratatui and Intuigram model types.
- `crates/rich-clipboard` is a reusable native clipboard-content library.
- `crates/test-harness` is a development-only hermetic behavior runner. It provides strict scripted adapters, separate real storage, semantic locators, and failure traces.

Use `intuigram-*` names for Intuigram-specific crates. Give reusable crates independent names. Do not create a crate only to group related types. A crate must hide important behavior behind a small interface at a demonstrated seam.

Domain-facing dependencies point to `intuigram-lib`. Adapter crates can use its Intuigram-owned values, but they must not depend on `intuigram-app`. `intuigram-app` is above `intuigram-lib` and the concrete adapters. The `intuigram` process package depends on Compio and `intuigram-app` in production. It creates the main-thread runtime with `#[compio::main]`. It owns command-line syntax and presentation. It sends a framework-free validated global-flags struct and command enum to the orchestration crate. Its behavior tests use `test-harness` as a development dependency. The harness depends on `intuigram-app` and on the adapter crates that it tests. `intuigram-app` and lower crates must not depend on `intuigram` or `test-harness`, including in development dependencies. Do not put adapter-specific values in `intuigram-lib`. These values include Ratatui widgets, Telegram TL constructors, SQLite rows, actor mailboxes, and platform clipboard types. Convert these values at the orchestration seams in `intuigram-app`. Do not create dependency cycles or shared catch-all type crates.

## State and concurrency

- The `intuigram-app` composition loop runs terminal input, rendering, synchronous `intuigram-lib` state reduction, result aggregation, and nonblocking platform effects on the main Compio runtime thread.
- An actor on a dedicated one-worker cluster constructs and owns each live Telegram Account session. It uses the upstream Compio `actor` feature. Its `LiveUpdates` driver is a retained worker-local task. Thus, Telegram invocations and update polling make progress together without moving non-`Send` connection state across threads.
- Only `intuigram-lib` owns mutable application state. It applies each typed input synchronously. It returns an immutable view and an optional adapter effect.
- The composition loop polls persistent event sources and a bounded set of correlated effect futures in place. Do not cancel and create an in-flight Compio operation again because a different source wakes. Cross-thread actor commands and normalized event output use bounded channels. Do not add channels between tasks on the same runtime thread.
- Terminal input and rendering must stay responsive while adapter effects are pending. Never run Telegram, database, media, clipboard, notification, or platform work synchronously in the terminal event loop.
- The TUI renders immutable snapshots or deltas.
- Blocking SQLite work stays on dedicated database threads. Other long adapter work stays asynchronous and returns typed results to the composition loop.
- Native clipboard reads, attachment validation and byte reads, media capture, notifications, external-link launch, completed-download launch, media decoding, cache access, and download writes stay outside the Telegram actor. Only Telegram upload and download byte transfer runs with the live client.
- Do not add shared mutable state across crates. Do not add mutex-protected application state.
- Specify backpressure, cancellation, ordering, and shutdown behavior.
- During shutdown, stop input admission. Cancel pending Telegram calls through the reserved cancellation path. Let Telegram updates that have returned complete their durable commit. Stop and join the actor and its worker-local update driver. Then join the actor cluster.

## Telegram and runtime

- Do not use `grammers-client`, `grammers-mtsender`, or `grammers-session` in the target architecture.
- Build raw Telegram behavior in `intuigram-telegram` on the small `compio-mtproto` interface.
- Initially keep mature low-level grammers crates for generated TL types, MTProto codecs, and cryptographic primitives. Do not rewrite cryptography or TL generation without a separate reviewed decision.
- Use the grammers sender implementation as a behavior reference for protocol state transitions, acknowledgements, retries, reconnection, data-center handling, and edge cases. Do not copy its Tokio-specific interface into Intuigram.
- Use Compio owned-buffer I/O for transport. Do not put Compio behind Tokio `AsyncRead` or `AsyncWrite` compatibility. This compatibility would remove the benefits of completion-based I/O.
- Direct Telegram TCP transport is p-core. Keep the transport seam ready for p-high SOCKS5, HTTP CONNECT, and MTProxy adapters.
- Convert Telegram TL values to Intuigram-owned data before they enter `intuigram-lib` or persistence.
- Unknown or new Telegram constructors must continue to synchronize. Show them as Unsupported Content. Do not drop them or crash the update loop.

## Persistence and filesystem

Use `rusqlite` and Refinery behind `intuigram-store`. Run blocking SQLite work on a dedicated database thread. Do not run it on an asynchronous executor thread.

Do not write SQL manually unless it is necessary. Prefer the existing typed persistence interfaces, migrations, and centralized query abstractions. If handwritten SQL is necessary, keep it in `intuigram-store`. Explain why the existing abstractions cannot do the operation. Add focused storage tests.

Use the platform configuration, data, cache, and download directories. Use this logical layout:

```text
<config>/intuigram/config.toml

<data>/intuigram/global.db
<data>/intuigram/.pending.db
<data>/intuigram/.pending.local-lock-salt
<data>/intuigram/<telegram-user-id>.db
<data>/intuigram/<telegram-user-id>.local-lock-salt

<cache>/intuigram/<telegram-user-id>/media/
<cache>/intuigram/<telegram-user-id>/thumbnails/
```

Figment can also load YAML, JSON, environment, and command-line sources. Do not store user configuration in SQLite only because a global database exists.

`global.db` contains only cross-Account runtime records. Each Account database contains its MTProto authorization and session state, synchronization cursors, synchronized records, Drafts, Draft History, search index, media metadata, and durable Outbox. Use the decimal Telegram user ID as the file name. Do not add a local UUID. A login starts in `.pending.db`. After Telegram gives the user ID, close the database and rename it atomically.

Atomically commit normalized Telegram data and its synchronization cursor before you show the durable result in the TUI. Save Draft changes before you report that they are saved. Represent local durability and Telegram acknowledgement separately.

Never silently delete, create again, or overwrite a database after corruption or migration failure. Migrations are embedded, versioned, and transactional. Back up the database before migration. Run integrity and foreign-key checks after migration. On failure, preserve original files and start the documented recovery procedure.

Use owner-only permissions for authorization and Account data. Never log authorization keys, API hashes, passwords, login codes, or Message bodies. Media Cache bytes are redownloadable. Local Records are not cache eviction candidates.

## TUI invariants

- Do not add keyboard modes or a flat focus cycle. Interaction uses the Chat list → Active Chat → Active Message hierarchy. Use modifier keys for lateral actions.
- Movement through the Chat list immediately changes the Active Chat and updates the adjacent Transcript. It does not enter the Chat. `Enter` moves into the Active Chat and activates its Composer by default. `Esc` moves up to the Chat list.
- The Composer stays the normal interaction target while a Chat is open, including after a send. `Alt+Up` moves from the Composer to the newest Message and then to older Messages. `Alt+Down` moves to newer Messages and returns to the Composer after the newest Message. `Esc` clears an Active Message and returns to the Composer before it moves up again.
- Folders are Chat-list navigation. They are not a focusable region. Bare `Left` and `Right` switch the Active Folder only when the Chat list is the interaction target. `Alt+Left` and `Alt+Right` are compatibility aliases in this context. Folder switching is not available from the Composer or an Active Message.
- Keep the context-sensitive Action Bar at the bottom, above the status bar. It shows all important actions that are currently available. `?` opens complete context help.
- Keep the Folder strip above the Action Bar.
- Generate the Action Bar and Help from the same effective keymap. Do not code displayed shortcuts separately from input handling.
- The current PoC visual design can require sufficient terminal space to show the Chat list and Active Chat side by side. Responsive stacking, narrow layouts, and resize preservation are p-high tasks.
- `Ctrl+F` is context-sensitive search. In the Active Chat, it searches that Chat. From Chat-list interaction, it searches the active Account globally.
- Use the OpenCode visual style. Do not use complete rectangular panel borders or titles in borders. Separate regions with whitespace, one-cell gutters, simple surfaces, plain bold headings, muted metadata, and one accent color.
- Show the current item or interaction target with a one-cell vertical rule on its left. Do not invert or replace the foreground and background colors for selection. The interface must stay legible with user terminal themes.
- Keep the Composer in the Active Chat column directly below the Transcript. Show it as a simple surface with a left accent rule. Show Draft, reply, edit, and attachment context inline. Do not show it as a titled box.
- Use a dense Transcript without Chat bubbles. Preserve Active Message, Message Selection, anchored scroll position, Draft, and interaction target during navigation.
- Do not move to the latest Message while the user reads old history. Show an explicit new-message indication.
- Advance Read State only when the Chat has focus and its newest Message is visible.
- Do not add manual refresh. Provide Reconnect only during Reconnect Cooldown.
- Keep useful text fallbacks for inline graphics and each Media Card.
- Never execute downloaded files. Use the documented safety behavior for suspicious links and launchable content.

## Rust conventions

- Use Rust 2024 edition and the toolchain from `nix develop`.
- Format with rustfmt. Keep Clippy clean with warnings denied during verification. Do not use crate-wide `#![deny(warnings)]`.
- Use `snafu` for error definition, propagation, and context. Each fallible module owns a module-scoped `Error` enum and `Result<T>` alias. Do not create a workspace-wide catch-all error enum. Reusable crates that explicitly return `std::io::Error`, such as `compio-term`, are exceptions when their public contract requires the OS error directly.
- Add semantic context at each module seam with SNAFU context selectors and `.context(...)`. Error variants must state the failed operation. Keep the lower-level source when it is useful.
- Convert dependency and adapter errors to the error type of the owning module before they cross its interface. Do not expose `rusqlite`, Compio, Telegram TL, Ratatui, clipboard, or other implementation errors through unrelated module interfaces.
- Use SNAFU `.context(...)` selectors when possible. Use `.with_context(...)` when selector context must be evaluated only after failure. Avoid `map_err`. Use it only when propagation needs a value-shape conversion that a SNAFU context selector cannot express clearly. Never use it only to rename or wrap an error.
- Do not use `anyhow`, `Box<dyn Error>`, opaque string errors, or unstructured SNAFU catch-all facilities in production interfaces. Model expected failure categories as explicit variants.
- Do not use `unwrap` in production paths. Use `expect` only for a real invariant. State the invariant in the message.
- Use newtypes where raw integers or strings from different domains can be confused. This includes Account, peer, Chat, Message, and request identifiers.
- Keep public interfaces small. Accept dependencies instead of constructing hidden globals. Return observable results instead of producing effects that tests cannot observe.
- Keep each handwritten source file below a soft limit of 200 lines and a hard limit of 400 lines. At more than 200 lines, review the file for cohesive module seams. Never exceed 400 lines. Keep `main.rs` and `lib.rs` small. They must declare modules, re-export the intentional interface, and do only top-level composition. Split files by behavior and ownership. Do not split at arbitrary line ranges. Generated sources and embedded schema fixtures are exceptions.
- When a Rust module owns child modules, use the `xxx/mod.rs` directory form. Do not use `xxx.rs` with an `xxx/` directory.
- Put related source files under a directory module with one cohesive interface. Prefer `backend/mod.rs` with private children to flat families such as `backend_*.rs`. Apply this rule when sibling files share a domain owner.
- Never use `include!` to join handwritten Rust source files. Use normal `mod` declarations, explicit imports and re-exports, and real module privacy boundaries.
- Put attributes that configure a complete module as inner attributes at the start of the source file for that module. Use attributes on a parent `mod` declaration only for conditions or paths that control loading of the child module.
- Document public items and important invariants. Do not add decorative section-divider comments.
- If one field in a struct or enum variant has an attribute or comment, put a blank line between all fields in that struct or variant. Otherwise, do not put blank lines between fields. If one enum variant has an attribute, put a blank line between all variants in that enum.
- Do not use wildcard imports outside test modules and intentional preludes.
- Do not optimize a possible hot path without measurements. Measure before you add performance complexity.

## Testing

Test modules through the same interfaces that callers use. Prefer deterministic tests with in-memory or temporary adapters. Normal tests must not require Telegram credentials or network access.

Name tests `<subject>_<scenario>_<outcome>` in `snake_case`. Use three to seven short domain words. Do not use filler words such as `test`, `should`, `when`, or `given`. Each test must verify an observable contract or focused invariant. It must fail for a possible defect. Do not test source text, wiring, constructor defaults, or other implementation details.

Keep cross-layer behavior scenarios under `crates/intuigram/tests/`. The top-level process package tests production composition through `test-harness` without reverse dependencies. Executable and PTY contract tests also belong there. They must start the real binary and use separate capability targets.

Required coverage includes:

- `intuigram-lib` state transitions and event ordering.
- `intuigram-app` composition, effect routing, synchronization, and shutdown ordering.
- MTProto framing, acknowledgement, retry, reconnection, salt, sequence, and data-center behavior with deterministic fixtures and a fake transport.
- Storage migrations from each released schema, backup and recovery behavior, transaction rollback, and cursor and data atomicity.
- Telegram constructor normalization, including fixtures for unknown constructors.
- TUI action resolution, effective key display, focus behavior, and the supported side-by-side PoC layout.
- Clipboard format priority and platform-adapter fallbacks without access to the real clipboard in unit tests.

Run the smallest applicable checks during development. Before you deliver a broad change, run these commands from the repository root. Prefer to run them in `nix develop`:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace --all-features
cargo test --workspace --doc --all-features
```

During the PoC-to-workspace transition, use the equivalent root-package command if a workspace command does not apply. State each environment or platform limit that prevents a check.

## Repository hygiene

- Preserve unrelated working-tree changes. The Nix and direnv files can contain user work in progress.
- Never commit credentials, local configuration, Account databases, Media Caches, recovery backups, or temporary login data.
- Do not edit generated TL sources manually. Update the schema input or generator.
- Do not update `Cargo.lock` unless dependency resolution changes.
- Keep each plain-text Markdown paragraph on one physical line. Preserve line breaks that define tables, lists, code blocks, and other Markdown structure.
- For commits, use exactly one Conventional Commit subject line. Do not add a body or co-author trailer. Do not push unless the user explicitly requests it.
- For a pull request, use the commit subject without a change as the pull request title. Keep the pull request body brief. Do not use emoji or too much Markdown in commit messages or pull requests.
