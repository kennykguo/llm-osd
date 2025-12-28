# llm-os

this repo implements a deterministic, auditable interface between a human (and eventually an llm) and a privileged os executor.

## repo structure

### rust workspace crates

- `llm-os-common/`
  - shared protocol types for the actionplan contract
  - strict json parsing (`deny_unknown_fields`)
  - deterministic validation caps and rules (anti-hallucination hardening)

- `llm-osd/`
  - privileged executor daemon (unix domain socket)
  - request parsing, validation, policy enforcement, action execution, json responses
  - append-only audit log (jsonl) with redaction

- `llmsh/`
  - client cli
  - local validation of plans
  - sending execute-mode plans to the daemon over the unix socket

## protocol overview

the system uses a single json document called an `ActionPlan`:

- includes `request_id`, optional `session_id`, `version`, `mode`, and `actions`
- `mode=plan_only` means no side effects (planning)
- `mode=execute` means the daemon attempts to execute actions

the daemon returns an `ActionPlanResult`:

- includes `request_id`, `executed`, `results[]`, optional `error`

the generated json schema lives at `docs/actionplan.schema.json`.

## documentation

key docs:

- `docs/PROMPT.md`: the prompt requirements for the project
- `docs/SUGGESTIONS.md`: design suggestions and constraints
- `docs/USAGE.md`: how to run `llm-osd` and use `llmsh`
- `docs/ERRORS.md`: deterministic error codes

## quick start

run the daemon:

```bash
cargo run -p llm-osd -- --socket-path /tmp/llm-osd.sock --audit-path ./llm-osd-audit.jsonl
```

ping it:

```bash
cargo run -p llmsh -- ping --socket-path /tmp/llm-osd.sock --session-id sess-1
```

send an execute plan:

```bash
echo '{"request_id":"req-echo-1","version":"0.1","mode":"execute","actions":[{"type":"exec","argv":["/bin/echo","hi"],"cwd":null,"env":null,"timeout_sec":5,"as_root":false,"reason":"test","danger":null,"recovery":null}]}' | cargo run -p llmsh -- send --socket-path /tmp/llm-osd.sock
```


