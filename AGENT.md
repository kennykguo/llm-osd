# agent guide

## code style

- rust code should match the surrounding file style.
- code comments (in source code) must be in lowercase.
- avoid unnecessary whitespace-only diffs.

## docs folder conventions

### 1) canonical docs (living files)

these files are the current source of truth and should not include timestamps in the contents:

- `docs/implementation_plan.md`
- `docs/task.md`
- `docs/USAGE.md`

when you update these, keep them current and consistent with the codebase.

### 2) snapshot docs (timestamped filenames)

when you want a point-in-time record, create a snapshot file whose filename includes the timestamp and do not put timestamps inside the file body.

filename convention:

- `NAME_YYYY-MM-DDTHH-MM-SS±TZ.md`

examples:

- `docs/SUMMARY_2025-12-27T21-28-33-05-00.md`
- `docs/implementation_plan_2025-12-27T21-28-33-05-00.md`
- `docs/task_2025-12-27T21-28-33-05-00.md`
- `docs/NEXT_IMPLEMENTATION_PLAN_2025-12-27T21-28-33-05-00.md`
- `docs/NEXT_TASK_2025-12-27T21-28-33-05-00.md`

### 3) doc naming

- prefer uppercase filenames only when intentionally chosen and used consistently (e.g. `USAGE.md`).
- if a doc is renamed (case changes), update references in other docs to match.

## README.md conventions

- `README.md` is the stable entry point and should describe the current structure of the repo.
- avoid embedding timestamps inside `README.md`.
- keep links to snapshot docs when they are useful for historical context.

## managing this file (AGENT.md)

- `AGENT.md` is for repo-wide conventions: code style, docs rules, and workflow rules.
- keep it short and operational.
- if you change conventions (like timestamp rules), update this file first.


