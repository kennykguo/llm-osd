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
cargo run -p llmsh -- ping --socket-path /tmp/llm-osd.sock
```

the response is json and includes `request_id`, `results`, and optional `error`.

## send a plan from stdin

```bash
echo '{"request_id":"req-echo-1","version":"0.1","mode":"execute","actions":[{"type":"exec","argv":["/bin/echo","hi"],"cwd":null,"env":null,"timeout_sec":5,"as_root":false,"reason":"test","danger":null,"recovery":null}]}' | cargo run -p llmsh -- send --socket-path /tmp/llm-osd.sock
```

notes:

- only allowlisted programs run without confirmation (mvp: `/bin/echo`)
- non-allowlisted programs require `confirmation.token`

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

## write a file

```bash
echo '{"request_id":"req-write-1","version":"0.1","mode":"execute","actions":[{"type":"write_file","path":"./tmp-llm-osd-write.txt","content":"hello\\n","mode":"0644","reason":"test","danger":null,"recovery":null}]}' | cargo run -p llmsh -- send --socket-path /tmp/llm-osd.sock
```

## audit log

the daemon appends one json object per line to the audit log path you pass.
each record includes top-level `request_id` and optional `session_id`.


