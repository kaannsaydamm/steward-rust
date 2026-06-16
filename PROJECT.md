# Project: Steward UI Ecosystem

## Architecture
- `crates/cli`: Contains the `steward-cli` binary. It acts as an interactive REPL/TUI, starts `steward-daemon` automatically if not running, displays a boot sequence, and communicates via gRPC to `steward-daemon`. Uses `ratatui` or `rustyline`.
- `crates/daemon`: Runs the `steward-daemon` process serving gRPC on `127.0.0.1:50051`. Provides gRPC-web and CORS for Web UI.
- `web-ui`: Next.js/React web dashboard connecting to `steward-daemon` via gRPC-web.
- `proto`: The gRPC protobuf definitions. Needs a new `Ping` RPC for the `status` command.

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| 1 | Daemon Concurrency & gRPC-Web | Implement gRPC-web and CORS in `steward-daemon` via `tonic-web` and `tower-http`. Ensure non-blocking async for multiple clients. | none | DONE |
| 2 | Advanced TUI Ecosystem | Update `steward-cli` with `ratatui` to provide an advanced terminal UI for monitoring and task execution. | M1 | PLANNED |
| 3 | Modern Web UI | Create a Next.js/React web app connecting to the daemon to show memory graph, tasks, and status. | M1 | PLANNED |

## Interface Contracts
### `cli` ↔ `daemon`
- **RPC**: `ExecuteTask(ExecuteTaskRequest)` -> `ExecuteTaskResponse`
- **RPC**: `RunPlugin(RunPluginRequest)` -> `RunPluginResponse`
- **RPC** (New): `Ping(PingRequest)` -> `PingResponse` (for `status` command)

## Code Layout
- `crates/cli/src/main.rs`: CLI entrypoint, REPL loop, Boot Sequence, Daemon spawn logic.
- `crates/daemon/src/main.rs`: Daemon entrypoint.
- `proto/steward.proto`: Protobuf definitions.
- `web-ui/`: Next.js web application directory.
