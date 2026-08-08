---
name: maintain-todo
description: Maintain a repository TODO.md as an actionable, prioritized checklist. Use when the user asks to add, reprioritize, promote all priorities, split, deduplicate, audit, or mark roadmap tasks complete, including invocations such as `$maintain-todo p-high ...` and `$maintain-todo promote`.
---

# Maintain TODO

Invoke as:

```text
$maintain-todo <priority-tag> <request>
$maintain-todo promote
```

## Priority

- Treat the priority tag as required except for `promote`.
- Accept only tags defined by the target `TODO.md`, such as `p-core`, `p-high`, `p-mid`, or `p-low`.
- Scope additions, updates, and completion audits to that priority.
- If the tag is missing, unknown, or ambiguous, ask the user which priority to use before editing. Do not infer it from urgency.

## Promote

For `$maintain-todo promote`, update every roadmap item exactly once from its
original priority:

- Keep `p-core` unchanged.
- Move `p-high` to `p-core`.
- Move `p-mid` to `p-high`.
- Move `p-low` to `p-mid`.

Do not cascade an item through multiple levels during one promotion. Preserve
checkbox state, wording, ordering, and unrelated content.

## Workflow

1. Locate the repository root and its `TODO.md`; prefer a path explicitly supplied by the user.
2. Read applicable repository instructions and the priority definitions in `TODO.md`.
3. Inspect implementation or documentation only as needed to verify completion claims. Skip implementation inspection for `promote`.
4. Edit only `TODO.md`. Do not change code, other documentation, or Git state unless explicitly requested separately.
5. Validate the resulting TODO diff with `git diff --check -- TODO.md` or the equivalent explicit path.

## Writing tasks

- Write an unchecked Markdown checkbox with the explicit priority tag.
- Begin with an imperative action and include concise acceptance criteria where useful.
- Put the task in the section used by the repository for that priority.
- Keep only actionable work in `TODO.md`; do not add background, status reports, observations about the current implementation, or architectural narration.
- Avoid duplicates. Merge overlapping tasks without weakening either requirement.
- Keep unrelated tasks and the repository's existing terminology intact.

## Completing tasks

- Mark `[x]` only when every clause in the task is verifiably complete.
- If an umbrella task is partially complete, split it into an exact completed `[x]` task and one or more remaining `[ ]` tasks.
- Never mark a partial implementation complete, silently weaken its wording, or discard unfinished acceptance criteria.
- When asked to mark finished work, audit all plausible tasks at the requested priority and briefly identify why any close candidates remain open.

## Report

Summarize tasks added, changed, split, completed, or promoted. State that only `TODO.md` was edited.
