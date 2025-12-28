# agent guide

## code style

- rust code should match the surrounding file style.
- code comments (in source code) must be in lowercase.
- avoid unnecessary whitespace-only diffs.

## docs folder conventions

### 1) doc naming

- documentation filenames in `docs/` use uppercase for stable names and snapshot prefixes.
- timestamps must be in the filename, not inside the file body.

### 2) snapshot docs (timestamped filenames)

snapshots are point-in-time records. do not include timestamps in the contents.

filename convention:

- `NAME_YYYY-MM-DDTHH-MM-SS±TZ.md`

examples:

- `docs/SUMMARY_2025-12-27T21-28-33-05-00.md`
- `docs/IMPLEMENTATION_PLAN_2025-12-27T21-28-33-05-00.md`
- `docs/TASK_2025-12-27T21-28-33-05-00.md`
- `docs/NEXT_IMPLEMENTATION_PLAN_2025-12-27T21-28-33-05-00.md`
- `docs/NEXT_TASK_2025-12-27T21-28-33-05-00.md`

### 3) canonical docs

the repo keeps these stable, non-timestamped docs:

- `docs/PROMPT.md`
- `docs/SUGGESTIONS.md`
- `docs/USAGE.md`
- `docs/ACTIONS.md`
- `docs/LOG.md`
- `docs/ERRORS.md`
- `docs/actionplan.schema.json`

the implementation plan and task tracking are maintained as timestamped snapshots:

- `docs/IMPLEMENTATION_PLAN_*.md`
- `docs/TASK_*.md`

when you update those, create a new snapshot file with a fresh timestamp.

### 4) duplicate cleanup

if duplicate files exist (often due to case-only differences), resolve them by:

- comparing contents (hash/diff)
- keeping the uppercase filename variant
- removing the duplicates

## README.md conventions

- `README.md` is the stable entry point and should describe the current structure of the repo.
- avoid embedding timestamps inside `README.md`.

## managing this file (AGENT.md)

- `AGENT.md` is for repo-wide conventions: code style, docs rules, and workflow rules.
- keep it short and operational.
- update this file first when conventions change.


