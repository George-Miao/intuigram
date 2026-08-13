---
name: maintain-todo
description: Maintain a repository TODO.md as an actionable prioritized checklist. Use this skill when the user asks to add, change priority, promote all priorities, split, remove duplicates, audit, or complete roadmap tasks. This includes requests such as `$maintain-todo p-high ...` and `$maintain-todo promote`.
---

# Maintain TODO

Use one of these commands:

```text
$maintain-todo <priority-tag> <request>
$maintain-todo promote
```

## Priority

- Require a priority tag except for `promote`.
- Accept only tags that the target `TODO.md` defines. Examples are `p-core`, `p-high`, `p-mid`, and `p-low`.
- Apply additions, updates, and completion audits only to that priority.
- If the tag is missing, unknown, or not clear, ask the user which priority to use before you edit. Do not infer priority from urgency.

## Promote

For `$maintain-todo promote`, update each roadmap item one time from its original priority:

- Keep `p-core` unchanged.
- Move `p-high` to `p-core`.
- Move `p-mid` to `p-high`.
- Move `p-low` to `p-mid`.

Do not move an item through more than one level during one promotion. Preserve checkbox state, wording, order, and unrelated content.

## Procedure

1. Locate the repository root and its `TODO.md`. Prefer a path that the user gives explicitly.
2. Read the repository instructions and the priority definitions in `TODO.md`.
3. Inspect implementation or documentation only as necessary to verify completion claims. Do not inspect implementation for `promote`.
4. Edit only `TODO.md`. Do not change code, other documentation, or Git state unless the user requests it separately.
5. Validate the TODO diff with `git diff --check -- TODO.md` or the equivalent explicit path.

## Write tasks

- Write an unchecked Markdown checkbox with an explicit priority tag.
- Start with an imperative action. Add concise acceptance criteria where useful.
- Put the task in the section for that priority.
- Keep only actionable work in `TODO.md`. Do not add background information, status reports, implementation observations, or architecture descriptions.
- Do not create duplicates. Combine overlapping tasks without removing requirements.
- Preserve unrelated tasks and repository terms.

## Complete tasks

- Mark `[x]` only when all parts of the task are verifiably complete.
- If part of an umbrella task is complete, split it into one exact completed `[x]` task and one or more remaining `[ ]` tasks.
- Never mark a partial implementation complete. Never reduce its requirements without an explicit instruction. Never remove incomplete acceptance criteria.
- When the user asks you to mark work complete, audit all plausible tasks at the specified priority. Briefly state why close candidates stay open.

## Report

Summarize the tasks that you added, changed, split, completed, or promoted. State that you edited only `TODO.md`.
