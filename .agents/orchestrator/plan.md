# Plan

1. Milestone 1: Proto Update & Daemon Status
   - Goal: Add a simple Ping RPC to gRPC to handle the `status` command from the CLI.
   - Files: `proto/steward.proto`, `crates/daemon/src/main.rs`.
   - Dispatch to a worker.

2. Milestone 2: CLI Lifecycle & TUI
   - Goal: Add rich boot sequence (`indicatif` / `inquire`), REPL loop (`rustyline` or standard IO / `inquire`), and daemon auto-spawning to `crates/cli`.
   - Files: `crates/cli/src/main.rs`, `crates/cli/Cargo.toml`.
   - Dispatch to a worker.

3. Final Verification
   - Verify all acceptance criteria manually.
