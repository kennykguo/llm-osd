# next implementation plan: llm-provider (llama.cpp local)

this plan outlines the next work: adding an `llm-provider` component that runs llama.cpp locally and emits schema-valid `ActionPlan` json for `llmsh`/`llm-osd`.

## goals

- local inference via llama.cpp
- llm is dormant by default (no always-on model process)
- llm output is constrained to the fixed `ActionPlan` schema (no arbitrary schemas)
- every generated plan is post-validated using `llm-os-common` before it can be forwarded to the daemon

## proposed architecture

add a new rust crate:

- `llm-provider` (bin)
  - reads a user prompt (stdin or `--prompt`)
  - runs llama.cpp on-demand (subprocess)
  - constrains decoding using a fixed grammar for the actionplan schema
  - outputs either:
    - `mode=plan_only` actionplan json (default), or
    - `mode=execute` actionplan json (only if explicitly requested)
  - always runs `parse_action_plan()` + `validate_action_plan()` on the model output

## constrained decoding strategy (fixed schema only)

we will not implement a general json-schema-to-cfg compiler.

instead, we will:

- write a hand-authored llama.cpp grammar (gbnf) for the `ActionPlan` schema
- keep it in-repo (suggested path: `llm-provider/grammar/actionplan.gbnf`)
- add unit tests that ensure:
  - the grammar file exists and contains the known action tags (exec/read_file/...)
  - model output that passes grammar also passes `llm-os-common` validation

## dormant-by-default mechanics

we will run llama.cpp as a short-lived subprocess per request:

- start llama.cpp
- run one generation
- exit

later, we can add an optional warm window, but only if you explicitly approve it.

## provider interface

the provider should produce only json on stdout.

suggested cli:

- `llm-provider generate --model-path ... --grammar-path ... --prompt-file ...`
- optional: `--mode plan_only|execute` (default: plan_only)
- optional: `--socket-path` to auto-send via `llmsh send` (or keep as a separate step)

## open decisions (need your confirmation)

1) llama.cpp entrypoint:
   - `llama-cli` subprocess, or
   - `llama-server` http, or
   - embedding llama.cpp as a library

2) model location and runtime:
   - where should the model file live on disk?
   - what default context length and threads should we use?

3) output workflow:
   - should the provider always emit `plan_only` first and require a second explicit step to convert to `execute`?
   - or do you want a single-step “generate+execute” path gated by the daemon confirmation token?


