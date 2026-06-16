# Scope: Advanced TUI Ecosystem

## Architecture
- `crates/cli` binary updated to use `ratatui` and `crossterm`.
- Replaces simple rustyline REPL with a full terminal UI.
- Features: Real-time daemon status indicator, task input bar, scrolling log/response window.

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| 1 | Ratatui Setup & Lifecycle | Refactor `crates/cli/src/main.rs` to initialize `ratatui` backend and handle terminal lifecycle (crossterm raw mode, panic hook). | none | PLANNED |
| 2 | UI Components & Event Loop | Implement the main event loop handling keyboard input. Add Status Bar, Log List, and Input Field components. | M1 | PLANNED |
| 3 | gRPC Integration | Connect UI actions to the `StewardServiceClient`. Update UI based on RPC responses. | M2 | PLANNED |

## Interface Contracts
### TUI ↔ Daemon
- **Endpoint**: `http://127.0.0.1:50051`
- **RPC**: `Ping` (poll for status)
- **RPC**: `ExecuteTask` (on input submit)

## Code Layout
- `crates/cli/src/main.rs`: Entrypoint, TUI setup, event loop.
- `crates/cli/src/app.rs`: Application state and logic.
- `crates/cli/src/ui.rs`: UI rendering functions.
