# llm-os implementation plan (mvp-first)

## mvp definition

deliver a minimal `llmsh` + `llm-osd` pair where:

- `llmsh` (user cli) sends a schema-valid json action plan to a local unix socket.
- `llm-osd` (privileged daemon) validates, audits, and executes a small allowlisted subset of actions.
- unknown fields and invalid values are rejected to harden against llm hallucinations.
- results are returned as structured json (typed per action).

## architecture options (required comparison)

| option | summary | risk | complexity | time-to-demo | security posture |
|---|---|---:|---:|---:|---:|
| linux control-plane (recommended) | deterministic linux kernel; llm drives a validated privileged daemon | low | low | fast | strong (cgroups/namespaces/audit/systemd) |
| xv6-in-qemu | toy kernel experimentation + minimal interface | medium | high | medium | weak until built |
| capability-first os (sel4-like) | research-grade isolation via capabilities | high | very high | slow | excellent but heavy lift |

recommendation: linux control-plane for mvp, keep xv6 path for kernel experimentation.

## deterministic interface contract

### envelope

an action plan is a single json document:

- `version`: protocol version string
- `mode`: `plan_only` or `execute`
- `actions`: ordered list of actions

### hallucination hardening

- schema parsing uses `deny_unknown_fields` everywhere
- per-action validators enforce required fields and bounds
- the executor enforces an allowlist and explicit confirmation policies for dangerous actions

### results

for each action, return:

- `ok`: boolean
- `stdout` / `stderr`: strings (with explicit truncation marker if truncated)
- `exit_code`: for exec
- `artifacts`: paths created/modified (when applicable)
- `error`: structured error info when `ok=false`

### auditing

every request/action is logged with:

- session id (from client)
- timestamp
- reason strings
- argv/cwd/env diffs
- before/after when applicable (mvp: limited to what we can capture cheaply)

## code structure for extensibility

### `llm-os-common`

shared types:

- request/response structs
- action enums
- strict json parse + serialize helpers
- validation helpers (pure functions)

### `llm-osd`

daemon responsibilities:

- unix socket server + framing
- request validation + policy
- action execution modules
- audit logging

layout (intended):

- `server`: accepts requests and routes actions
- `policy`: allowlist + confirmation gates
- `actions/*`: one module per capability (exec, filesystem, services, cgroups, devices, etc.)
- `audit`: append-only log writer

### `llmsh`

client responsibilities:

- collect user request / llm output
- send action plan to daemon
- display structured results

## llm serving strategies (design-level; not required for mvp code)

| strategy | mechanism | “llm dormant” compliance |
|---|---|---|
| local on-demand | systemd socket activation or per-request spawn | strong |
| remote endpoint | https request on demand | local dormant, remote not |
| hybrid | small local router + remote big model | strong locally |

## mvp milestones

### milestone 1: protocol + strict parsing

exit criteria:

- unknown fields are rejected (tests)
- request/response types exist in `llm-os-common`

### milestone 2: daemon socket + exec action

exit criteria:

- `llm-osd` listens on unix socket
- `llmsh` can send an exec request and receive results
- audit log records requests and results

### milestone 3: file io actions

exit criteria:

- `read_file` bounded by `max_bytes`
- `write_file` with explicit mode

### milestone 4: policy + confirmation plumbing

exit criteria:

- risk tagging for actions
- daemon refuses dangerous actions without explicit confirmation token

notes:

- mvp policy also supports requiring confirmation for specific exec programs even if `danger` is unset (example: `rm`)


