# llm-os implementation plan (mvp-first)

## mvp definition

deliver a minimal `llmsh` + `llm-osd` pair where:

- `llmsh` (user cli) sends a schema-valid json action plan to a local unix socket.
- `llm-osd` (privileged daemon) validates, audits, and executes a small allowlisted subset of actions.
- unknown fields and invalid values are rejected to harden against llm hallucinations.
- results are returned as structured json (typed per action).
- include a `ping` action for deterministic health checks without exec.

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

- `request_id`: required identifier for correlation
- `session_id`: optional identifier for correlation
- `version`: protocol version string
- `mode`: `plan_only` or `execute`
- `actions`: ordered list of actions

### hallucination hardening

- schema parsing uses `deny_unknown_fields` everywhere
- per-action validators enforce required fields and bounds
- validator caps number of actions per plan and caps exec argv sizes
- validator caps exec env sizes
- the executor enforces an allowlist and explicit confirmation policies for dangerous actions
- exec is allowlisted by default; non-allowlisted programs require confirmation
- exec.as_root is rejected by validation in the mvp
- daemon uses an idle read timeout so it does not rely on client EOF to complete a request
- read_file/write_file for absolute paths outside `/tmp/` require confirmation
- read_file/write_file paths containing `..` require confirmation

### results

for each action, return:

- `ok`: boolean
- `stdout` / `stderr`: strings (with explicit truncation marker if truncated)
- `stdout_truncated` / `stderr_truncated`: booleans
- `exit_code`: for exec
- `artifacts`: paths created/modified (when applicable)
- `error`: structured error info when `ok=false`

request-level failures (parse/validation/mode/size) return:

- `error.code`: deterministic string enum
- `error.message`: human-readable detail

per-action failures return:

- `results[].*.error.code`: deterministic string enum
- `results[].*.error.message`: human-readable detail

read_file semantics:

- daemon reads at most `max_bytes` (plus one extra byte for truncation detection)
- `truncated=true` means the file had more than `max_bytes` bytes
- `max_bytes` is capped by validation (mvp: 65536)

write_file semantics:

- `content` is capped by validation (mvp: 65536 bytes)

### auditing

every request/action is logged with:

- request id (from client)
- session id (from client)
- timestamp
- reason strings
- argv/cwd/env diffs
- before/after when applicable (mvp: limited to what we can capture cheaply)

notes:

- audit redacts confirmation tokens, exec env values, and write_file content

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


