# llm-os mvp running task

## current goal

build a deterministic, auditable interface between a human, an llm, and the os executor such that llm hallucinations cannot directly cause arbitrary execution.

## non-negotiable constraints

- llm output must be schema-valid json for execution.
- the executor must reject unknown fields and invalid values.
- every privileged operation must be logged with who/why/what.
- support clean extension for new os capabilities (e.g. monitor, keyboard, mouse) via a stable action envelope and per-capability modules.

## current status

- wip branch created and snapshotted: `wip/2025-12-27-mvp`
- added `llm-os-common` shared protocol crate with strict parsing (`deny_unknown_fields`) and a failing test proving hallucination rejection

## next steps

- restore `docs/implementation_plan.md` with the architecture + mvp milestones
- define `llm-os-common` response types and error model (typed results, truncation markers)
- implement `llm-osd` unix socket server that:
  - accepts an action plan json
  - validates it and applies policy
  - executes allowlisted actions (`exec`, `read_file`, `write_file`)
  - returns structured results and writes an audit log
- implement `llmsh` cli client that:
  - sends a plan to `llm-osd`
  - prints results deterministically (json)


