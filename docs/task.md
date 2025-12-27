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
- added explicit confirmation token support in the actionplan schema and daemon-side enforcement for policy-sensitive exec (mvp: rm)
- added required `request_id` to action plans and echoed it in daemon responses for deterministic correlation
- added `plan_only` mode support in the daemon with an `executed` boolean in responses
- added additional action types with deterministic plan_only responses (no side effects):
  - `service_control`
  - `install_packages`
  - `remove_packages`
  - `update_system`
  - `observe`
  - `cgroup_apply`
  - `firmware_op`

## next steps

none right now; mvp core is implemented. see `docs/usage.md` for how to run it.


