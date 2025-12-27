# llm-os mvp usage

this is a minimal walkthrough for running `llm-osd` and talking to it with `llmsh`.

## run the daemon

in one terminal:

```bash
cargo run -p llm-osd -- --socket-path /tmp/llm-osd.sock --audit-path ./llm-osd-audit.jsonl
```

the confirmation token is configurable:

```bash
cargo run -p llm-osd -- --socket-path /tmp/llm-osd.sock --audit-path ./llm-osd-audit.jsonl --confirm-token custom-token
```

## ping (no exec)

in another terminal:

```bash
cargo run -p llmsh -- ping --socket-path /tmp/llm-osd.sock --session-id sess-1
```

the response is json and includes `request_id`, `results`, and optional `error`.
the response also includes `executed` so callers can distinguish `plan_only` from `execute`.

## service_control (plan_only)

this returns `executed=false` and a structured result describing what would run.

```bash
echo '{"request_id":"req-plan-svc-1","version":"0.1","mode":"plan_only","actions":[{"type":"service_control","action":"status","unit":"ssh.service","reason":"inspect service status","danger":null,"recovery":null}]}' | cargo run -p llmsh -- send --socket-path /tmp/llm-osd.sock
```

## install_packages (plan_only)

this returns `executed=false` and a structured result describing what would run.

```bash
echo '{"request_id":"req-plan-pkg-1","version":"0.1","mode":"plan_only","actions":[{"type":"install_packages","manager":"apt","packages":["curl","git"],"reason":"install tools","danger":null,"recovery":null}]}' | cargo run -p llmsh -- send --socket-path /tmp/llm-osd.sock
```

## remove_packages (plan_only)

this returns `executed=false` and a structured result describing what would run.

```bash
echo '{"request_id":"req-plan-rmpkg-1","version":"0.1","mode":"plan_only","actions":[{"type":"remove_packages","manager":"apt","packages":["curl","git"],"reason":"remove tools","danger":null,"recovery":null}]}' | cargo run -p llmsh -- send --socket-path /tmp/llm-osd.sock
```

## send a plan from stdin

```bash
echo '{"request_id":"req-echo-1","version":"0.1","mode":"execute","actions":[{"type":"exec","argv":["/bin/echo","hi"],"cwd":null,"env":null,"timeout_sec":5,"as_root":false,"reason":"test","danger":null,"recovery":null}]}' | cargo run -p llmsh -- send --socket-path /tmp/llm-osd.sock
```

you can also override correlation fields without editing the json:

```bash
echo '{"request_id":"req-echo-1","version":"0.1","mode":"execute","actions":[{"type":"exec","argv":["/bin/echo","hi"],"cwd":null,"env":null,"timeout_sec":5,"as_root":false,"reason":"test","danger":null,"recovery":null}]}' | cargo run -p llmsh -- send --socket-path /tmp/llm-osd.sock --session-id sess-1
```

## local validation (no daemon)

validate a plan without sending it to the daemon:

```bash
echo '{"request_id":"req-validate-1","version":"0.1","mode":"plan_only","actions":[{"type":"ping"}]}' | cargo run -p llmsh -- validate
```

notes:

- only allowlisted programs run without confirmation (mvp: `/bin/echo`)
- non-allowlisted programs require `confirmation.token`
- `exec.as_root` is rejected by validation in the mvp
- validator caps number of actions per plan and exec argv sizes
- validator caps exec env sizes
- validator caps request_id/session_id/reason/path sizes
- validator caps version/token/danger/recovery/mode sizes
- validator caps exec.timeout_sec (mvp: 60s)
- daemon rejects requests larger than 256kiB

## confirmation token

example: run a non-allowlisted program (mvp uses `/usr/bin/true`) by providing the confirmation token:

```bash
echo '{"request_id":"req-true-1","version":"0.1","mode":"execute","actions":[{"type":"exec","argv":["/usr/bin/true"],"cwd":null,"env":null,"timeout_sec":5,"as_root":false,"reason":"test","danger":null,"recovery":null}],"confirmation":{"token":"i-understand"}}' | cargo run -p llmsh -- send --socket-path /tmp/llm-osd.sock
```

## read a file (base64)

```bash
echo '{"request_id":"req-read-1","version":"0.1","mode":"execute","actions":[{"type":"read_file","path":"./Cargo.toml","max_bytes":4096,"reason":"test","danger":null,"recovery":null}]}' | cargo run -p llmsh -- send --socket-path /tmp/llm-osd.sock
```

the daemon returns `content_base64` and `truncated`.

notes:

- the daemon only reads up to `max_bytes` (plus one extra byte to detect truncation), so `read_file` does not load large files into memory.
- `max_bytes` is capped by validation (mvp: 65536).
- absolute paths outside `/tmp/` require a confirmation token.
- paths containing `..` require a confirmation token.

## write a file

```bash
echo '{"request_id":"req-write-1","version":"0.1","mode":"execute","actions":[{"type":"write_file","path":"./tmp-llm-osd-write.txt","content":"hello\\n","mode":"0644","reason":"test","danger":null,"recovery":null}]}' | cargo run -p llmsh -- send --socket-path /tmp/llm-osd.sock
```

notes:

- absolute paths outside `/tmp/` require a confirmation token.
- paths containing `..` require a confirmation token.
- `content` is capped by validation (mvp: 65536 bytes).

## audit log

the daemon appends one json object per line to the audit log path you pass.
each record includes top-level `request_id` and optional `session_id`.
audit redacts confirmation tokens, exec env values, and write_file content.

## actionplan json schema

the repo includes a generated json schema for the actionplan protocol:

- `docs/actionplan.schema.json`
- `docs/error_codes.md` lists deterministic error codes returned by the daemon

to regenerate it from the rust types:

```bash
cargo run -p llm-os-common --bin actionplan_schema > docs/actionplan.schema.json
```


