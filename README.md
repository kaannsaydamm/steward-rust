# Steward

Steward is a local-first Agent Harness OS: a Rust daemon, an operator CLI/TUI, a
knowledge layer, and a web UI for running multi-phase agent workflows with
durable memory.

The project is designed around a simple premise: the operator should be able to
start a workflow, inspect what the agents are doing, approve execution, recover
after daemon restarts, and keep useful lessons in memory without leaving the
terminal.

## Current Capabilities

- Rust daemon with gRPC and gRPC-web endpoints
- Interactive Hermes-style terminal shell when `steward-cli` is launched without
  a subcommand
- Non-interactive CLI commands for automation and smoke tests
- Multi-phase workflow runner with approval gates
- Durable workflow state and workflow events in SQLite
- Workflow resume after daemon restart
- Workflow status and agent log inspection from CLI and TUI
- Long-term, reasoning, negative, and dream memory types
- Nightly dream report support for memory consolidation
- Web UI with workflow, agent, terminal, and knowledge graph views
- E2E tests that run the real daemon and real CLI binaries

## Product Phases

Steward started as a UI ecosystem build, but the scope has grown into a compact
Agent Harness OS. This is the working phase map for the current codebase:

| Phase | Name | Status | What It Means |
|---|---|---|---|
| 0 | Core daemon and proto | Done | gRPC service, protobuf contracts, SQLite task storage, basic daemon lifecycle |
| 1 | Multi-client daemon gateway | Done | gRPC-web, CORS, async daemon serving CLI and web clients together |
| 2 | Operator CLI and Hermes TUI | Done | Ratatui/crossterm shell, command rail, transcript, prompt editing, history, scrollback |
| 3 | Workflow engine | Done | Multi-phase workflow runner, approval gate, agent logs, CLI/TUI inspection |
| 4 | Durable control plane | Done | SQLite workflow/event persistence, daemon restart recovery, resumed approval flow |
| 5 | Memory substrate | Done | Long-term, reasoning, negative lesson, and dream memory types |
| 6 | Nightly consolidation | Done | `--dream-now`, dream directory output, midnight scheduler for memory reports |
| 7 | Web operator surface | Done | Next.js dashboard for workflows, agents, terminal, and knowledge graph views |
| 8 | Live operation loop | In progress | Follow/watch workflow progress, surface logs continuously, tighten operator feedback |
| 9 | Tool execution and skills | Next | Real tool registry, command execution policies, skill packs, MCP/tool adapters |
| 10 | Packaging and install | Next | Release binaries, service install, config profiles, update path, smaller disk footprint |
| 11 | Production hardening | Next | Retention/pruning, auth/policy, audit trail, crash recovery, deeper web/TUI parity |

The current implementation is strongest in phases 0-7. Phase 8 is where the
operator experience becomes more like a real long-running agent console rather
than a set of one-shot commands.

## Workspace Layout

```text
crates/
  cli/          Rust operator CLI and interactive TUI
  core/         Protobuf-generated shared service types
  daemon/       Steward daemon, workflow runtime, persistence, nightly jobs
  knowledge/    Memory, vector, graph, and hybrid retrieval layer
proto/          gRPC service definition
tests/          Cross-crate e2e tests using real binaries
web-ui/         Next.js operator web interface
```

## Requirements

- Rust toolchain
- Node.js and npm for the web UI
- Windows, macOS, or Linux shell capable of running the Rust binaries

On Windows, Git Bash or PowerShell both work for normal CLI usage. The
interactive TUI has been smoke-tested in a Windows PTY.

## Build

```bash
cargo build -p steward-cli -p steward-daemon
```

Build the web UI:

```bash
cd web-ui
npm install
npm run build
```

## Run The Daemon

```bash
cargo run -p steward-daemon -- --port 50051
```

The daemon stores its local SQLite database as `steward.db` in the working
directory unless started from another directory.

Useful daemon flags:

```bash
steward-daemon --port 50051
steward-daemon --dream-now
steward-daemon --dream-dir memory/nightly
```

## Use The CLI

Ping the daemon:

```bash
cargo run -p steward-cli -- --host http://127.0.0.1:50051 ping
```

Open the interactive operator shell:

```bash
cargo run -p steward-cli -- --host http://127.0.0.1:50051
```

Inside the shell:

```text
/ping
/status
/agents
/workflows
/workflow <title>
/watch <workflow_id>
/inspect <workflow_id>
/logs <workflow_id> <agent_id>
/approve <workflow_id>
/cancel <workflow_id>
/memory
/remember <text>
/lesson <scope> | <error> | <correction>
/recall [query]
/dreams [query]
/clear
/quit
```

Interactive controls:

```text
Up / Down          command history
Left / Right       edit prompt
Home / End         jump in prompt
Delete / Backspace edit prompt
PageUp / PageDown  scroll transcript
Ctrl-L             clear transcript
Ctrl-U             clear prompt
Esc / Ctrl-C       exit
```

## Workflow Example

Start a workflow and stop at approval:

```bash
steward-cli --host http://127.0.0.1:50051 workflow start \
  --description "ship a compact agent harness" \
  "agent harness implementation"
```

Inspect it:

```bash
steward-cli --host http://127.0.0.1:50051 workflow status <workflow_id>
```

Watch its progress:

```bash
steward-cli --host http://127.0.0.1:50051 workflow watch <workflow_id>
```

Read an agent log:

```bash
steward-cli --host http://127.0.0.1:50051 workflow logs <workflow_id> architect
```

Approve execution:

```bash
steward-cli --host http://127.0.0.1:50051 workflow approve <workflow_id>
```

The workflow runner persists state and events to SQLite. If the daemon restarts
while a workflow is waiting for approval, the workflow can be approved after the
restart and will continue from the persisted state.

## Memory

Store long-term memory:

```bash
steward-cli --host http://127.0.0.1:50051 memory remember "Prefer compact Rust modules."
```

Store a negative lesson:

```bash
steward-cli --host http://127.0.0.1:50051 memory lesson \
  --scope workflow-runtime \
  --error "runner stopped when client stream closed" \
  --correction "continue state machine even if response stream receiver drops"
```

Recall memory:

```bash
steward-cli --host http://127.0.0.1:50051 memory recall workflow
steward-cli --host http://127.0.0.1:50051 memory dreams nightly
```

## Verification

Run Rust unit and doc tests:

```bash
cargo test -p steward-cli -p steward-core -p steward-daemon -p steward-knowledge
```

Run e2e tests:

```bash
cargo test -p steward-e2e-tests --test e2e -- --test-threads=1
cargo test -p steward-e2e-tests --test workflow_persistence -- --test-threads=1
```

Run web checks:

```bash
cd web-ui
npm run lint
npm run build
```

## Design Notes

Steward is intentionally local-first:

- SQLite is the durable control plane for workflow and memory state.
- The CLI is the primary operator surface.
- The daemon owns long-running workflow state and recovery.
- The web UI is a companion operator surface, not a replacement for the CLI.

The current implementation is moving toward a compact production harness. The
next major hardening areas are packaging, service installation, live event
following, richer tool execution, policy controls, and deeper web/TUI parity.
