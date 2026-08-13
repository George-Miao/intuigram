# Intuigram repository guidance

## General Rules

- Keep this file accurate when the repository structure, toolchain, architecture, or required checks change.
- Communicate with the user in English.
- Add comments only when they explain non-obvious intent or constraints.
- Add tests when they verify real behavior or guard a regression; do not add placeholder tests.

## Documentation Style

- Prefer active voice. Name the type or component that performs an action, for example, “`RetryLayer` retries failed operations.”
- Use direct, present-tense sentences.
- Lead with what a public type or method does, then explain important constraints, defaults, and behavior.
- Describe API semantics precisely. Verify option types, capability requirements, error behavior, and overwrite or versioning semantics against the implementation.
- Keep terminology consistent with the codebase.
- Use parallel structure in lists and punctuate complete sentences consistently.

## Project direction

Intuigram is a local-first Telegram terminal client intended to replace a primary GUI client for routine communication. The current implementation is a Rust virtual workspace built around a single-writer application reducer, a Compio orchestration loop, dedicated per-Account Telegram actors, isolated SQLite Account storage with optional SQLCipher Local Lock, a dense Ratatui TUI, and a hierarchical process CLI. Calls and Secret Chats remain outside the current Daily Driver promise.

Update `TODO.md` at the end of a task to match the current state. Leave it unchanged when the task is not represented there.

## Required reading

Before architectural or product work, read:

1. `CONTEXT.md` for the project vocabulary and settled behavioral meanings.
2. `TODO.md` for scope and the `p-core`, `p-high`, `p-mid`, and `p-low` priorities.
3. Every relevant decision under `docs/adr/`.

Treat these documents as the current source of truth over `README.md`. When a decision changes, update all affected documents in the same change. Keep `CONTEXT.md` implementation-free; add an ADR only for a consequential architectural decision with a real tradeoff.

## Target workspace

The target is a virtual Cargo workspace. Every package belongs under `crates/`.

Declare every dependency used by any workspace crate in the root `[workspace.dependencies]` table. Member manifests must reference normal, development, build, target-specific, path, and Git dependencies with `workspace = true`; keep dependency versions and shared defaults centralized, while adding only genuinely crate-specific feature selections at the member.

- `crates/intuigram`: executable entrypoint, Compio runtime startup, colorful Clap command hierarchy, and cross-layer behavior tests. It owns process argument parsing, defaults to starting the TUI when no subcommand is supplied, and passes validated launch arguments downward to `intuigram-app`.
- `crates/intuigram-app`: application orchestration, adapter composition, synchronization, recovery, and the main Compio runtime loop. Its modules live directly under `src/`; do not restore a redundant `src/application/` wrapper.
- `crates/intuigram-lib`: sole owner of canonical application state, synchronous transitions, user intents, adapter events, effects, and read-only view data.
- `crates/intuigram-tui`: terminal input, adaptive layout, and rendering.
- `crates/intuigram-telegram`: Telegram login, synchronization, raw requests, update reconciliation, and translation to Intuigram-owned data.
- `crates/intuigram-store`: durable application records, migrations, backups, and recovery.
- `crates/intuigram-media`: media transfer, cache policy, and media lifecycle.
- `crates/intuigram-config`: layered Figment configuration.
- `crates/compio-mtproto`: reusable Compio-based MTProto connection, session, invocation, and update-stream library.
- `crates/compio-term`: experimental reusable Compio-native terminal event readiness; keep its API explicitly unstable until the Windows backend and cross-platform behavior are resolved.
- `crates/rasterm`: reusable terminal raster-image detection, cell geometry, protocol encoding, external-renderer commands, and image lifecycle; keep it free of Ratatui and Intuigram model types.
- `crates/rich-clipboard`: reusable native clipboard-content library.
- `crates/test-harness`: dev-only hermetic behavior runner with strict scripted adapters, isolated real storage, semantic locators, and failure traces.

Use `intuigram-*` for Intuigram-specific crates. Give genuinely reusable crates independent names. Do not create a crate merely to group related types. A crate must hide meaningful behavior behind a small interface at a demonstrated seam.

Domain-facing dependencies point toward `intuigram-lib`; adapter crates may use its Intuigram-owned values but must not depend on `intuigram-app`. `intuigram-app` sits above `intuigram-lib` and the concrete adapters. The `intuigram` process package depends on Compio and `intuigram-app` in production; it creates the main-thread runtime with `#[compio::main]`, owns command-line syntax and presentation, and passes a framework-free validated global-flags struct plus command enum into the orchestration crate. Its behavior tests use `test-harness` as a development dependency; the harness depends downward on `intuigram-app` and the adapter crates it exercises. Neither `intuigram-app` nor any lower crate may depend on `intuigram` or `test-harness`, including through development dependencies. Keep adapter-specific values out of `intuigram-lib`, including ratatui widgets, Telegram TL constructors, SQLite rows, actor mailboxes, and platform clipboard types. Translate those values at the orchestration seams in `intuigram-app`. Avoid dependency cycles and shared catch-all type crates.

## State and concurrency

- The `intuigram-app` composition loop runs terminal input, rendering, synchronous `intuigram-lib` state reduction, result aggregation, and nonblocking platform effects on the main Compio runtime thread.
- Each live Telegram Account session is constructed and owned by an actor on a dedicated one-worker cluster using Compio's upstream `actor` feature. Its `LiveUpdates` driver is a retained worker-local task so Telegram invocations and update polling make progress together without moving non-`Send` connection state across threads.
- `intuigram-lib` exclusively owns mutable application state and applies each typed input synchronously, returning an immutable view and optional adapter effect.
- The composition loop polls persistent event sources and a bounded set of correlated effect futures in place. Do not cancel and recreate an in-flight Compio operation merely because another source wakes. Cross-thread actor commands and normalized event output use bounded channels; do not add channels between tasks that remain on the same runtime thread.
- Terminal input and rendering must remain responsive while adapter effects are pending. Never execute Telegram, database, media, clipboard, notification, or platform work synchronously in the terminal event loop.
- The TUI renders immutable snapshots or deltas.
- Blocking SQLite work stays on dedicated database threads. Other long-running adapter work remains asynchronous and returns typed results to the composition loop.
- Native clipboard reads, attachment validation and byte reads, media capture, notifications, external-link launches, completed-download launches, media decoding, cache access, and download writes stay outside the Telegram actor. Only Telegram upload/download byte transfer runs with the live client.
- Do not introduce cross-crate shared mutable state or mutex-protected application state.
- Make backpressure, cancellation, ordering, and shutdown behavior explicit.
- On shutdown, stop accepting input, cancel pending Telegram calls through the reserved cancellation path, let already returned Telegram updates finish their durable commit, stop and join the actor and its worker-local update driver, then join the actor cluster.

## Telegram and runtime

- Do not use `grammers-client`, `grammers-mtsender`, or `grammers-session` in the target architecture.
- Build raw Telegram behavior in `intuigram-telegram` on the small `compio-mtproto` interface.
- Initially retain mature low-level grammers crates for generated TL types, MTProto codecs, and cryptographic primitives. Do not rewrite cryptography or TL generation without a separate reviewed decision.
- Use grammers' sender implementation as a behavioral reference for protocol state transitions, acknowledgements, retries, reconnection, data-center handling, and edge cases. Do not copy its Tokio-specific interface into Intuigram.
- Use Compio owned-buffer I/O for the transport. Do not hide Compio behind Tokio `AsyncRead` or `AsyncWrite` compatibility that defeats completion-based I/O.
- Direct Telegram TCP transport is p-core. Keep the transport seam ready for p-high SOCKS5, HTTP CONNECT, and MTProxy adapters.
- Telegram TL values must be normalized into Intuigram-owned data before crossing into `intuigram-lib` or persistence.
- Unknown or newly introduced Telegram constructors must remain synchronizable and appear as Unsupported Content rather than being dropped or crashing the update loop.

## Persistence and filesystem

Use `rusqlite` and Refinery behind `intuigram-store`. Run blocking SQLite work on a dedicated database thread rather than an asynchronous executor thread.

Do not write SQL manually unless it is absolutely necessary. Prefer existing typed persistence interfaces, migrations, and centralized query abstractions. When handwritten SQL is unavoidable, keep it isolated inside `intuigram-store`, explain why the existing abstractions cannot express the operation, and cover it with focused storage tests.

Use the platform config, data, cache, and download directories. The logical layout is:

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

Figment may also load YAML, JSON, environment, and command-line sources. Do not store user configuration in SQLite merely because a global database exists.

`global.db` contains only cross-Account runtime records. Each Account database contains its MTProto authorization and session state, synchronization cursors, synchronized records, Drafts, Draft History, search index, media metadata, and durable Outbox. Use the decimal Telegram user ID as the filename; do not add a local UUID. A login starts in `.pending.db`, then closes and atomically renames that database after Telegram reveals the user ID.

Commit normalized Telegram data and its synchronization cursor atomically before exposing the durable result to the TUI. Persist Draft changes before reporting them saved. Represent local durability and Telegram acknowledgement separately.

Never silently delete, recreate, or overwrite a database after corruption or migration failure. Migrations are embedded, versioned, transactional, backed up before execution, and followed by integrity and foreign-key checks. Preserve original files and enter the documented recovery flow on failure.

Enforce owner-only permissions for authorization and Account data. Never log authorization keys, API hashes, passwords, login codes, or Message bodies. Media cache bytes are redownloadable; Local Records are not cache eviction candidates.

## TUI invariants

- Do not add keyboard modes or a flat focus cycle. Interaction follows the Chat list → Active Chat → Active Message hierarchy and uses modifier keys for lateral actions.
- Moving through the Chat list immediately changes the Active Chat and updates the adjacent Transcript without entering the Chat. `Enter` descends into the Active Chat with its Composer active by default; `Esc` ascends to the Chat list.
- The Composer remains the ordinary interaction target while a Chat is open, including after sending. `Alt+Up` moves from the Composer to the newest Message and toward older Messages; `Alt+Down` moves toward newer Messages and returns to the Composer after the newest. `Esc` clears an Active Message and returns to the Composer before ascending further.
- Folders are Chat-list navigation rather than a focusable region. Bare `Left` and `Right` switch the Active Folder only while the Chat list is the interaction target; `Alt+Left` and `Alt+Right` remain compatibility aliases there. Folder switching is unavailable from the Composer or an Active Message.
- Keep the context-sensitive Action Bar at the bottom, above the status bar. It shows all important actions currently available; `?` opens exhaustive context help.
- Keep the Folder strip above the Action Bar.
- Generate the Action Bar and Help from the same effective keymap. Do not hardcode displayed shortcuts separately from input handling.
- The current PoC visual pass may assume enough terminal space to show the Chat list and Active Chat side by side. Responsive stacking, narrow layouts, and resize preservation are p-high follow-up work.
- `Ctrl+F` is context-sensitive search. Anywhere inside the Active Chat searches that Chat; Chat-list interaction searches the active Account globally.
- Use an OpenCode-inspired visual language: avoid full rectangular panel borders and titles embedded in borders; separate regions with whitespace, one-cell gutters, restrained surfaces, plain bold headings, muted metadata, and a single accent color.
- Indicate the current item or interaction target with a one-cell vertical rule at its left. Do not invert or replace both foreground and background colors for selection; the interface must remain legible under user terminal themes.
- Keep the Composer in the Active Chat column directly below the Transcript. Present it as a restrained surface with a left accent rule and inline Draft, reply, edit, and attachment context rather than as a titled box.
- Use a dense Transcript without chat bubbles. Preserve Active Message, Message Selection, anchored scroll position, Draft, and interaction target across navigation.
- Do not snap to the latest Message while the user reads older history. Show an explicit new-message affordance.
- Advance Read State only when the Chat has focus and its newest Message is visible.
- Do not add manual refresh. Offer Reconnect only during Reconnect Cooldown.
- Preserve useful text fallbacks for inline graphics and every Media Card.
- Never execute downloaded files. Suspicious links and launchable content require the documented safety treatment.

## Rust conventions

- Use Rust 2024 edition and the toolchain provided by `nix develop`.
- Format with rustfmt and keep Clippy clean with warnings denied in verification, not through crate-wide `#![deny(warnings)]`.
- Use `snafu` for error definition, propagation, and context. Each fallible module owns a module-scoped `Error` enum and `Result<T>` alias; do not create a workspace-wide catch-all error enum.
- Add semantic context at every module seam with SNAFU context selectors and `.context(...)`. Error variants must explain the operation that failed and retain the lower-level source where useful.
- Translate dependency and adapter errors into the owning module's error type before they cross its interface. Do not expose `rusqlite`, Compio, Telegram TL, ratatui, clipboard, or other implementation errors through unrelated module interfaces.
- Use SNAFU `.context(...)` selectors whenever possible, and use `.with_context(...)` when constructing the selector's context should be evaluated lazily only on failure. Avoid `map_err`; use it only when propagation requires a genuine value-shape transformation that a SNAFU context selector cannot express clearly, never merely to rename or wrap an error.
- Do not use `anyhow`, `Box<dyn Error>`, opaque string errors, or SNAFU's unstructured catch-all facilities in production interfaces. Model expected failure categories as explicit variants.
- Do not use `unwrap` in production paths. Use `expect` only for a genuine invariant and explain that invariant in the message.
- Use newtypes where raw integers or strings from different domains could be confused, especially Account, peer, Chat, Message, and request identifiers.
- Keep public interfaces small. Accept dependencies rather than constructing hidden globals, and return observable results rather than producing untestable side effects.
- Keep each handwritten source file below a soft limit of 200 lines and a hard limit of 400 lines. Crossing 200 lines should trigger a deliberate review for cohesive module seams; never cross 400 lines. Keep `main.rs` and `lib.rs` especially small: they should declare modules, re-export the intentional interface, and perform only top-level composition. Split by behavior and ownership, not arbitrary line ranges. Generated sources and embedded schema fixtures are exempt.
- When a Rust module owns child modules, use the directory form with `xxx/mod.rs`; do not pair `xxx.rs` with an `xxx/` directory.
- Group logically related source files beneath a directory module with one cohesive interface. Prefer `backend/mod.rs` with private children over flat filename families such as `backend_*.rs`, and apply the same rule whenever sibling files share a domain owner.
- Never use `include!` to splice Rust source files together. Use ordinary `mod` declarations, explicit imports and re-exports, and real module privacy boundaries.
- Put attributes that configure a whole module as inner attributes at the start of that module's own source file. Reserve attributes on a parent's `mod` declaration for conditions or paths that control whether and where Rust loads the child module.
- Document public items and non-obvious invariants. Do not add decorative section-divider comments.
- When any field in a struct has an attribute or comment, leave a blank line between every field in that struct. When any enum variant has an attribute, leave a blank line between every variant in that enum.
- Avoid wildcard imports outside test modules and deliberate preludes.
- Do not optimize speculative hot paths. Add measurements before performance-specific complexity.

## Testing

Test modules through the same interfaces callers use. Prefer deterministic tests with in-memory or temporary adapters; ordinary tests must not require Telegram credentials or network access.

Name tests `<subject>_<scenario>_<outcome>` in `snake_case`, using three to seven short domain words without filler such as `test`, `should`, `when`, or `given`. Every test must defend an observable contract or focused invariant and fail under a plausible defect; do not test source text, wiring, constructor defaults, or other incidental implementation details.

Keep cross-layer behavior scenarios under `crates/intuigram/tests/`, where the top-level process package exercises production composition through `test-harness` without reverse dependencies. Executable and PTY contract tests also belong there, but must launch the actual binary and remain separate capability targets.

Required coverage includes:

- `intuigram-lib` state transitions and event ordering.
- `intuigram-app` composition, effect routing, synchronization, and shutdown ordering.
- MTProto framing, acknowledgement, retry, reconnection, salt, sequence, and data-center behavior using deterministic fixtures and a fake transport.
- Storage migrations from every released schema, backup/recovery behavior, transaction rollback, and cursor/data atomicity.
- Telegram constructor normalization, including fixtures for unknown constructors.
- TUI action resolution, effective key display, focus behavior, and the supported side-by-side PoC layout.
- Clipboard format precedence and platform-adapter fallbacks without accessing the real clipboard in unit tests.

Run the narrowest relevant checks while iterating. Before handing off a broad change, run from the repository root, preferably inside `nix develop`:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace --all-features
cargo test --workspace --doc --all-features
```

During the proof-of-concept-to-workspace transition, use the equivalent root-package command when a workspace command is not yet applicable. State clearly when environment or platform limitations prevent a check.

## Repository hygiene

- Preserve unrelated working-tree changes. The Nix and direnv files may be user-owned work in progress.
- Never commit credentials, local configuration, Account databases, media caches, recovery backups, or temporary login data.
- Do not edit generated TL sources manually; update their schema input or generator.
- Do not update `Cargo.lock` unless dependency resolution genuinely changed.
- Keep each plain-text Markdown paragraph on one physical line; preserve line breaks that define tables, lists, code blocks, and other structured Markdown.
- When creating commits, use exactly one Conventional Commit subject line; do not add a body or co-author trailer. Do not push unless explicitly requested.
- When creating a pull request, reuse the commit subject unchanged as the pull request title and keep the body to a brief description. Never use emoji or excessive Markdown formatting in commit messages or pull requests.
