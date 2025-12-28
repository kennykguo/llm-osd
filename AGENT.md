# agent guide

## code style

- rust code should match the surrounding file style.
- code comments (in source code) must be in lowercase.
- avoid unnecessary whitespace-only diffs.

## docs folder conventions

### 1) docs folder layout

`docs/` is structured into:

- `docs/internal/`: stable reference docs (schema, usage, prompt, errors, etc.)
- `docs/summary/`: timestamped historical summaries (what is already done)
- `docs/task/`: timestamped task snapshots
- `docs/implementation/`: timestamped implementation snapshots

the newest task + implementation snapshots are the current source of truth.

### 2) doc naming

- documentation filenames use uppercase for stable names and snapshot prefixes.
- timestamps must be in the filename, not inside the file body.

### 3) snapshot docs (timestamped filenames)

snapshots are point-in-time records. do not include timestamps in the contents.

filename convention:

- `NAME_YYYY-MM-DDTHH-MM-SS±TZ.md`

examples:

- `docs/summary/SUMMARY_2025-12-27T21-28-33-05-00.md`
- `docs/task/TASK_2025-12-27T21-27-33-05-00.md`
- `docs/implementation/IMPLEMENTATION_2025-12-27T21-28-33-05-00.md`

### 4) operating rule: always read the latest task + implementation first

before doing any implementation work, always read:

- the latest `docs/task/TASK_*.md`
- the latest `docs/implementation/IMPLEMENTATION_*.md`

use older snapshots only for historical context.

### 5) internal docs

the repo keeps stable, non-timestamped docs in `docs/internal/` (examples):

- `docs/internal/PROMPT.md`
- `docs/internal/SUGGESTIONS.md`
- `docs/internal/USAGE.md`
- `docs/internal/ACTIONS.md`
- `docs/internal/LOG.md`
- `docs/internal/ERRORS.md`
- `docs/internal/actionplan.schema.json`

### 6) duplicate cleanup

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


