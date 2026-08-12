---
name: implement-feature
description: Implement Intuigram product features through its real application, adapter, persistence, keymap, and renderer seams with hermetic behavior coverage. Use when adding or changing user-visible behavior, completing a TODO.md feature, or fixing a cross-layer behavior regression in this repository.
---

# Implement Feature

Build user-visible behavior test-first through the repository's hermetic behavior runner. Keep lower-level unit tests where they are the sharper oracle.

## Workflow

1. Read `AGENTS.md`, `CONTEXT.md`, `TODO.md`, every relevant ADR, and `docs/research/e2e-behavior-test-infrastructure.md`. Preserve unrelated worktree changes.

2. Identify the exact behavior and its roadmap priority. Use the explicit priority from the request or matching TODO item; ask only when a required priority is genuinely ambiguous.

3. Add or change a capability-oriented integration-test target under `crates/intuigram/tests/` before implementation. Extend the `test-harness` crate only when the scenario demonstrates a missing reusable test seam.

4. Drive actions through real Crossterm input using `TestSystem::press`, `type_text`, `paste`, `resize`, or focus events. Observe behavior through live semantic locators and representative rendered cells. Do not mutate `intuigram-lib` state directly in a behavior scenario.

5. Script Telegram work with strict typed `TelegramScenario` expectations. Use the real temporary SQLite Account database. Make held work, completion order, disconnects, and updates explicit.

6. Run the narrow scenario and confirm it fails for the missing behavior. Implement the smallest coherent production change, then rerun until green.

7. Run the complete hermetic target:

   ```sh
   cargo nextest run -p intuigram
   ```

   If Nextest is unavailable, use `cargo test -p intuigram`. Run one scenario while iterating with `cargo test -p intuigram --test <capability>`. Then run the narrow owning-crate checks and the repository's full verification gate for broad changes.

8. When every clause of a roadmap item is verifiably complete, use `$maintain-todo <priority> mark the matching item complete`. Do not mark partial work complete.

9. After that, use `$commit-changes` to commit the feature.

## Harness rules

- Never use sleeps, live Telegram Accounts, production captures, hidden retries, or timing windows.
- Never weaken strict mock expectations to make a failure green.
- Keep locators user-facing and domain-based: Chat, Message, Composer, Folder, and Action.
- Record deterministic time and seed when a scenario depends on them. Control races with named held work and explicit completions.
- Prefer semantic assertions. Use cell assertions for layout, style, clearing, and renderer-contract behavior.
- Add PTY coverage only for executable/TTY facts that the in-process runner cannot prove.
- A harness failure must retain the concise mismatch and its machine-readable trace path.
- End each successful scenario with `expect_no_unhandled_work()`.

## Extending support

Keep `TestSystem` synchronous and small even though production I/O is asynchronous. Add typed fixture builders rather than YAML or stringly behavior scripts. If a feature needs a new adapter family, introduce the production seam first and substitute only at that seam. Add a harness self-test whenever new waiting, matching, tracing, cleanup, or mock behavior could produce a false green.
