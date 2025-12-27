# Architecture Proposal: llm-os

## Goal Description
To design and implement `llm-os`, a system where an LLM acts as a privileged operator for a Linux-based OS, driven by explicit user intent and structured interfaces. The system ensures the LLM is dormant by default, safe, and auditable.

## User Review Required
> [!IMPORTANT]
> **Architecture Choice**: We are proceeding with the **Linux-based Control Plane** approach. This avoids the complexity of writing a kernel from scratch while providing real OS capabilities.
>
> **Language Stack**: Proposing **Rust** for the privileged daemon (`llm-osd`) for memory safety and **Rust** for the CLI/TUI (`llmsh`) to share schema definitions.
>
> **LLM Serving**: Proposing a **Hybrid** approach (Local small model for fast routing/formatting + Remote/On-demand Large model for complex tasks) or **Local On-Demand** via socket activation.
>
> **Coding Standards**:
> - **Scalability**: Code structure must support easy extension for new hardware/features (monitor, keyboard, mouse).
> - **Comments**: All code comments must be in **lowercase format**.

## Proposed Architecture

### 1. OS Substrate: Linux Control Plane
We will build on top of a standard Linux distribution (e.g., Ubuntu/Debian or a minimal distro).
- **Kernel**: Standard Linux Kernel.
- **Privileged Daemon (`llm-osd`)**:
    - Runs as `root`.
    - Listens on a secure Unix socket.
    - Validates and executes structured actions.
    - Enforces policy (allowlist, confirmation).
    - Logs to system audit logs (`journald`).
- **User Interface (`llmsh`)**:
    - Runs as `user`.
    - Accepts user prompts.
    - Invokes the LLM (local or remote).
    - Parses LLM output into structured JSON.
    - Sends JSON plans to `llm-osd` for execution.

### 2. LLM Serving Strategy
**Primary Strategy: Local On-Demand (Socket Activated)**
- **Mechanism**: Use `systemd` socket activation to start the inference server (e.g., `llama.cpp` server) only when `llmsh` connects.
- **Idle Timeout**: Configure the server to exit after N seconds of inactivity to satisfy "dormant by default".
- **Fallback**: Option to use remote APIs (OpenAI/Anthropic) for "Brainstorm Mode" or complex planning, configured via user preference.

### 3. Structured Interface Contract
**Schema**: JSON-based Action Plan.
**Validation**:
- **LLM Side**: Use `llguidance` (or grammar constraints) to force valid JSON output.
- **OS Side**: `llm-osd` strictly validates the JSON schema and checks values against allowlists (e.g., allowed paths, allowed packages).

**Action Types (MVP)**:
- `exec`: Run command (allowlisted binaries only initially).
- `read_file`: Read file content (size limited).
- `write_file`: Write content (user confirmation required for overwrite).
- `install_packages`: Wrapper for `apt`/`dnf`.

### 4. Privilege Model & Isolation
- **Executor**: `llm-osd` is the only component with root.
- **Sandboxing**:
    - Risky `exec` operations can be run inside `systemd-run` transient units with restricted capabilities or namespaces.
    - Future: Run `llm-osd` itself in a container or VM if higher isolation is needed.

## MVP Implementation Plan

### Milestone 1: Core Skeleton
- [ ] **Project Setup**: Rust workspace with `llm-osd` (daemon) and `llmsh` (cli).
- [ ] **Daemon**: Basic Unix socket listener, simple "echo" action.
- [ ] **CLI**: Prompt input, send to daemon, print response.
- [ ] **Schema**: Define `Action` struct shared between crates.

### Milestone 2: Basic Actions & Security
- [ ] **Actions**: Implement `exec`, `read_file`, `write_file`.
- [ ] **Policy**: Implement "Confirmation Required" logic in `llm-osd`.
- [ ] **Logging**: Structured logging to `stdout`/`journald`.

### Milestone 3: LLM Integration
- [ ] **Inference**: Integrate with a local `llama.cpp` server or remote API.
- [ ] **Grammar**: Apply JSON schema constraints to LLM generation.
- [ ] **End-to-End**: User prompt -> LLM Plan -> JSON -> `llm-osd` Execution.

### Milestone 4: System Control
- [ ] **Package Management**: `install_packages` wrapper.
- [ ] **Process Control**: Basic `cgroup` or `systemctl` wrappers.

## Verification Plan

### Automated Tests
- **Unit Tests**: Rust tests for schema serialization/deserialization.
- **Integration Tests**:
    - Spin up `llm-osd` in a test harness.
    - Send mock JSON requests via socket.
    - Assert on responses and side effects (e.g., file creation).
    - *Note*: Will need to run some tests as root or use `fakeroot`/namespaces for CI.

### Manual Verification
- **End-to-End Demo**:
    1. Start `llm-osd` (sudo).
    2. Run `llmsh`.
    3. Type: "Create a file named hello.txt with content 'world'".
    4. Verify LLM generates correct JSON.
    5. Verify `llmsh` prompts for confirmation.
    6. Verify file is created.
