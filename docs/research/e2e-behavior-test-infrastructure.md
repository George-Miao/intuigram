# End-to-end behavior test infrastructure

Research snapshot: 2026-08-03

## Conclusion

Intuigram should build a Playwright-like developer experience, but not by driving every test through a pseudo-terminal. The reliable default should be a hermetic, in-process behavior runner that composes the real input resolver, application state owner, adapter orchestration, SQLite store, and Ratatui renderer around deterministic test adapters and an in-memory terminal. A much smaller second tier should launch the actual binary in a pseudo-terminal and parse its ANSI output to verify the terminal contract. Live Telegram Test-DC checks belong in a third, explicitly non-gating conformance job.

This split gives coding agents one cheap, deterministic command for behavioral feedback without pretending that an in-memory renderer verifies raw-mode setup, escape encoding, signal-driven resize, or terminal restoration. Conversely, it keeps OS scheduling, child-process cleanup, and terminal-emulator differences out of the hundreds of tests that do not need them.

The recommendation below is an architectural proposal, not a report that this infrastructure already exists. Today Intuigram has strong state and rendering tests and a few composition tests, but no reusable end-to-end runner.

## What “like Playwright” should mean here

Playwright's useful ideas are clean-slate fixtures, live locators, auto-waited actions and assertions, controlled external dependencies, and a trace that explains a failure. Playwright creates an isolated browser context for each test to prevent state and failure carry-over, and its fixtures give each test only the environment it requests ([isolation](https://playwright.dev/docs/browser-contexts), [fixtures](https://playwright.dev/docs/test-fixtures)). Its locators are resolved again when used and prioritize user-facing roles and names; actions and web-first assertions wait for their preconditions instead of requiring sleeps ([locators](https://playwright.dev/docs/locators), [auto-waiting](https://playwright.dev/docs/actionability), [assertions](https://playwright.dev/docs/test-assertions)). Playwright can also replace external API responses, control application time, and record a step-by-step trace ([API mocking](https://playwright.dev/docs/mock), [clock](https://playwright.dev/docs/clock), [trace viewer](https://playwright.dev/docs/trace-viewer-intro)).

The corresponding Intuigram concepts should be:

| Playwright concept  | Intuigram equivalent                                                                                                                                       |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Browser context     | A fresh `TestSystem`: new runtime, channels, temporary config/data/cache/download roots, database, adapters, terminal, clock, and trace                    |
| Page                | `AppDriver`, owning one running application and its latest rendered frame                                                                                  |
| Locator             | A lazy query over the latest semantic UI tree, such as `chat("Rust")`, `composer()`, `message(id)`, or `action(Action::Send)`                              |
| User action         | A real Crossterm event/key chord delivered through Intuigram's actual resolver; no direct state mutation                                                   |
| Web-first assertion | An assertion re-evaluated after relevant app/render/adapter notifications until it passes or reaches a diagnostic deadline                                 |
| Route/HAR mock      | A strict scripted Telegram adapter at the Intuigram-owned command/event seam, plus byte/TL fixtures in lower-level protocol tests                          |
| Clock               | An injected wall/monotonic clock whose timers advance only when the test says so                                                                           |
| Trace viewer        | A failure bundle containing numbered actions, renders, adapter commands/events, storage commits, virtual-time advances, pending work, and the final screen |

Tests should still use `CONTEXT.md` vocabulary: Chat, Active Chat, Message, Composer, Draft, Folder, and Current Action.

## Proposed test layers

### 1. Hermetic behavioral E2E: the primary feedback loop

Use the dev-only `test-harness` crate to hide application composition, strict adapters, locators, assertions, isolated storage, and traces behind a small interface. The top-level `intuigram` package includes it only as a development dependency, while the harness depends downward on the production composition, state, and adapter crates it exercises. Neither `intuigram-app` nor a lower crate depends on the harness or process package, so the complete package graph remains acyclic. Put each capability scenario directly under `crates/intuigram/tests/` as an independent integration-test target. Cargo integration tests are separate crates that exercise public package interfaces ([Cargo test targets](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#tests)).

The runner should use the production composition entry point, not reproduce the loops in test code. Its substitutions are only at intended adapter seams:

- a scripted Telegram service instead of the network client;
- the real `intuigram-store` against per-test temporary roots;
- Ratatui `TestBackend` at a fixed terminal size;
- fake media, clipboard, notification, link-launch, and download adapters;
- injected clocks, entropy/random-ID sources, and terminal capabilities.

Ratatui describes `TestBackend` as an in-memory backend intended for integration tests of an entire terminal UI. It exposes the cell buffer, cursor, resize, and buffer assertions ([Ratatui `TestBackend`](https://docs.rs/ratatui/0.30.2/ratatui/backend/struct.TestBackend.html)). That is the right default rendering boundary. Behavioral actions should enter as Crossterm `Event` or an Intuigram `KeyChord`, so the real context-sensitive keymap is covered. Direct `Intent` injection remains valuable for `intuigram-lib` tests, but it is not an end-to-end user action.

The TUI should produce a semantic tree during the same layout/render pass: role, user-facing name, stable domain ID, state, and bounds. Locators re-run against the latest tree. This avoids brittle coordinates but does not replace cell verification: renderer self-tests must relate nodes to cells, and representative scenarios should snapshot normalized grids and styles.

Actions wait for input acceptance and the resulting rendered revision. Expectations re-evaluate only after view, render, adapter, or virtual-clock notifications. A wall deadline only bounds hangs. “Settled” means runnable work and channels are drained, the latest view is rendered, and remaining work is explicitly held by a gate, timer, or request—not that a quiet period elapsed.

### 2. PTY/VT terminal contract: thin black-box coverage

Keep a small `crates/intuigram/tests/pty/` suite for facts the in-process runner cannot establish:

- the compiled binary starts on a TTY, enters raw/alternate-screen state, and restores it after normal exit and error;
- raw legacy and enhanced-keyboard byte sequences become the expected actions;
- bracketed paste, resize, focus events, cursor visibility, and full redraws survive the real terminal pipeline;
- `--demo`, configuration/path overrides, errors, and shutdown work at the executable boundary.

Cargo automatically builds binary targets for integration tests and exposes the absolute path as `CARGO_BIN_EXE_<name>` ([Cargo environment variables](https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-cargo-test)). Use `portable-pty` to create a native PTY at an explicit size, spawn that binary, write input, and resize it. Its master API exposes read, write, and resize, and its child handle supports nonblocking status checks and termination ([`MasterPty`](https://docs.rs/portable-pty/0.9.0/portable_pty/trait.MasterPty.html), [`Child`](https://docs.rs/portable-pty/0.9.0/portable_pty/trait.Child.html)). Parse the captured byte stream with `vt100`, which turns terminal bytes into an in-memory screen and exposes cells and terminal state ([`vt100`](https://docs.rs/vt100/0.16.2/vt100/)).

PTY tests should assert eventual terminal states and process status, never byte chunk boundaries: PTY reads may split or combine writes. A guard must terminate and reap the child and retain raw bytes on every failure. The current `compio-term` Windows input backend is not implemented, so initially require this tier on Linux and macOS and add Windows only with that backend.

### 3. Telegram Test-DC conformance: useful, never ordinary E2E

Scheduled or manually triggered tests may authenticate reserved Telegram test accounts against Test DCs to validate the real handshake, login migration, and a few representative RPC/update paths. Telegram provides reserved number prefixes and fixed codes specifically for authorization testing, warns not to put private information in those accounts because anybody can use them, and recommends proving authorization on Test DCs before production ([Telegram test accounts](https://core.telegram.org/api/auth#test-accounts)).

These checks are external conformance probes, not required pull-request feedback: network reachability, Telegram service state, flood control, and test data resets are outside the repository's control. They must use separate credentials, never production DCs or a personal Account, produce sanitized logs, and report “external unavailable” separately from a product assertion failure. Protocol determinism belongs in `compio-mtproto` fake-transport tests; Telegram constructor normalization belongs in fixture tests; ordinary behavior belongs in the hermetic runner.

## Deterministic data and Telegram mocking

The fake Telegram service should implement the production Intuigram-owned command/event interface and be strict: unexpected, duplicate, wrongly ordered, unmatched, or unused work fails teardown. Scripts must bootstrap stable typed domain data; expect commands and return success, errors, or held gates; inject updates, gaps, reconnects, and reordered completions; separate server acknowledgement from local durability; and fault either side of a storage commit.

Prefer typed Rust fixture builders for scenarios. They are compiler-checked, reuse Intuigram domain types, and allow small defaults. Use committed external files only where bytes are the subject: raw TL constructors, MTProto envelopes, media samples, and released database schemas. Do not create a YAML behavior DSL that becomes a second, weakly typed programming language. Do not replay captured production sessions: encrypted MTProto contains dynamic message IDs, salts, session state, and secrets, and real captures are both unstable and unsafe.

Fix wall/monotonic time, locale, timezone, terminal capabilities, random seed, and platform directories per test. Drive timers with the fake clock and include the entropy seed/index in traces. Use real SQLite in a fresh temporary root, because durability, transactions, and restart behavior are product behavior.

## Proposed Rust test API

Keep the public test surface small and synchronous even if the system it drives is asynchronous. `TestSystem` owns and pumps the runtime; callers describe user actions and observable outcomes.

```rust
#[test]
fn pending_reply_is_acknowledged_after_reconnect() -> TestResult {
    let mut app = TestSystem::builder()
        .terminal(100, 24)
        .time("2026-08-03T12:00:00Z")
        .telegram(
            TelegramScenario::new()
                .bootstrap(account("Ada").with_chat(chat(10, "Rust")))
                .expect_load_history(10, messages([incoming(40, "Lin", "hello")]))
                .hold_send_text("send", 10, "on it", Some(40)),
        )
        .start()?;

    app.screen().chat("Rust").expect_active()?;
    app.press(key::ENTER)?;
    app.press(key::ALT_UP)?;
    app.press(key::CTRL_R)?;
    app.type_text("on it")?;
    app.press(key::ENTER)?;

    app.screen()
        .message_text("on it")
        .expect_delivery(DeliveryState::Pending)?;

    app.telegram().disconnect()?;
    app.telegram().reconnect()?;
    app.telegram().complete("send", sent_message(41))?;

    app.screen()
        .message(41)
        .expect_delivery(DeliveryState::Sent)?;
    app.expect_no_unhandled_work()?;
    Ok(())
}
```

The exact names can change, but these properties should not:

- `screen().chat(...)` and other locators are lazy and enforce exactly one match unless the caller explicitly asks for a collection;
- actions use real effective keys and report why an action is unavailable;
- expectations retry only on observable state changes and show the last value;
- fake-adapter controls are explicit test steps, not background timing;
- teardown verifies consumed expectations, stopped tasks, closed channels, terminal restoration, and no writes outside the temporary roots.

Suggested organization:

```text
crates/intuigram/tests/
  {harness,navigation,drafts,messaging,synchronization,reconnect,recovery}.rs

crates/test-harness/src/
  {lib,system,telegram,clock,screen,trace,error}.rs

crates/intuigram/tests/pty.rs
crates/intuigram/tests/pty/
  {lifecycle,input,resize}.rs
```

Use capability-oriented files and names that read as behavior, not one file per implementation module. Keep raw protocol, store migration, and renderer unit tests in their owning crates.

## Test the test framework

A false-green harness is more dangerous than a missing assertion. Give the test-support module its own fixture application and conformance suite before using it as a gate:

1. **Action delivery:** keys, paste, resize, and focus arrive exactly once; unavailable actions fail visibly.
2. **Waiting:** a state change passes only after its gate opens; a closed gate times out; quiet output never counts as progress.
3. **Locators:** pre-render locators find replacement nodes; zero/multiple matches produce useful failures.
4. **Traces:** intentional matcher, adapter, timeout, panic, and shutdown errors retain steps, screens, pending work, time, and seed in a tested schema.
5. **Mocks:** unexpected, unused, reordered, duplicate, and late work fails.
6. **Isolation/cleanup:** concurrent tests cannot share state; every exit path stops actors, reaps children, restores the terminal, and retains artifacts.
7. **Renderer/VT parity:** production frames encoded and parsed through the VT path match normalized `TestBackend` cells, cursor, color, and resize state.
8. **Sensitivity:** controlled dropped/reordered events and altered UI state turn the intended test red, proving the oracle can observe real regressions.

Do not make the framework and product share assertions. The product may share layout metadata used to render; the test-support layer owns matching, waiting, diagnostics, and teardown. Cell-level rendering tests and lower-level state tests remain an independent oracle for the semantic runner.

## Flake prevention and diagnosis policy

The required suite should adopt these rules as policy, not advice:

1. **No sleeps.** Synchronize on gates, notifications, revisions, child status, or fake-clock advances; wall deadlines only bound hangs.
2. **No hidden retries.** Required checks run once; a diagnostic retry that passes still fails CI. Nextest supports `--flaky-result fail` ([nextest retries](https://nexte.st/docs/features/retries/)).
3. **Fresh isolation.** Share no database, directory, port, mutable static, or test order. Nextest also isolates process globals per test ([why process-per-test](https://nexte.st/docs/design/why-process-per-test/)).
4. **Deterministic ordering.** Tests control races with named gates; randomized schedules always print and accept a replay seed.
5. **Strict teardown.** Unexpected and leaked work fails. Nextest can terminate hangs and fail detectable subprocess leaks ([timeouts](https://nexte.st/docs/features/slow-tests/), [leaky tests](https://nexte.st/docs/features/leaky-tests/)); PTY guards still own cleanup because not every leak is detectable.
6. **Explain waits.** Timeouts identify the condition and events that failed to progress, not merely “timed out.”
7. **Snapshots are secondary.** Prefer semantic assertions; snapshot changes require review and CI uses `INSTA_UPDATE=no` ([Insta workflow](https://insta.rs/docs/quickstart/)).
8. **Stress detects; retries do not forgive.** Use nextest's stress mode ([stress tests](https://nexte.st/docs/features/stress-tests/)); quarantine only with a named owner and expiry, never by adding green-making retries.

Record traces in memory and flush on first failure. Redact credentials and non-fixture content. Include build/platform, terminal, virtual time, seed, numbered actions/events, revisions, final tree/screen, pending work, and exit status.

## Reliable feedback for coding agents

Expose one obvious required command and one narrow filter path:

```sh
cargo nextest run -p intuigram
cargo nextest run -p intuigram --test messaging pending_reply
cargo nextest run -p intuigram --test pty
```

The first command should need no credentials or network and finish fast enough to run after each behavioral change. Test names and failure steps should use domain language so an agent can map a failure back to `CONTEXT.md`. On failure, print the concise semantic mismatch and the artifact path; do not flood stdout with every successful render. Provide an `inspect` command later that renders a saved trace step-by-step, but keep the trace format documented and machine-readable so agents can inspect it without a GUI.

Agents should be instructed to add or change a behavioral test before changing the behavior, run the narrow module while iterating, then run the full behavior target. They must not use a live Account, add sleeps/retries, weaken strict mock expectations, or auto-accept snapshots. PTY tests are added only when the behavior concerns the executable/terminal boundary; most product behavior belongs in the hermetic tier.

## Current seams and blockers

The first hermetic slice now exposes production input resolution, in-memory rendering with semantic nodes, a synchronous application driver, strict Telegram scenarios, isolated real Account storage, live locators, and failure traces through `test-harness`. Remaining rollout work includes:

- production code calls `SystemTime::now`, `compio::time::sleep`, and `getrandom::fill` directly, so time, retries, expiry, and outbound random IDs cannot yet be controlled from a behavior scenario;
- actual-binary PTY/VT lifecycle coverage and opt-in Telegram Test-DC conformance remain separate later tiers.

The solution is not to make all internals public. Keep a small library-level composition interface in `intuigram-app`; give it explicit production adapter, terminal, clock, entropy, and filesystem dependencies; expose a test renderer entry point or `TestTerminal` implementation; and introduce a transport trait at the `compio-mtproto` behavior seam. Keep adapter-specific types out of `intuigram-lib` and translate all test-support/dependency errors into module-scoped SNAFU errors, consistent with repository rules.

## Rollout

1. Extract production orchestration from private `main.rs` into a small public surface. Replace timeout-window-based observations with gates and notifications.
2. Inject clock, timer, entropy, platform directories, and adapter factories. Add view/render revision acknowledgements and a deterministic pump.
3. Build the integration-test support module with one strict Telegram scenario, temporary real store, `TestBackend`, trace, and the complete self-test suite above. Extract a crate later only if reuse justifies it without a cycle.
4. Port three vertical scenarios first: open Chat and preserve focus; send a reply through pending/acknowledged states; reconnect without blocking input.
5. Add semantic nodes/locators and representative cell snapshots. Port behavior only when the scenario adds cross-layer coverage; retain narrow unit tests.
6. Add a handful of actual-binary PTY/VT lifecycle, input, paste, and resize tests on supported Unix platforms. Prove cleanup and trace behavior with intentional harness faults.
7. Add opt-in Telegram Test-DC conformance after the fake transport and normalization suites are mature. Report it separately and never let an external outage invalidate the hermetic signal.

The acceptance bar for the first slice is deliberately strict: repeat the required behavior suite hundreds of times with no failure, run tests in varying order and parallelism with identical results, prove that every deliberate harness fault is detected, and make every failure reproducible from its trace and seed. Only then should it become the default feedback loop for agents.
