# BRIEFING — 2026-06-11T11:57:00Z

## Mission
Refactor `crates/cli/src/main.rs` to initialize `ratatui` backend and handle terminal lifecycle.

## 🔒 My Identity
- Archetype: Implementer
- Roles: implementer, qa, specialist
- Working directory: c:\Users\kaluclu\Desktop\steward\.agents\teamwork_preview_worker_m1_1
- Original parent: 82f8bb16-201a-4493-bf88-99b62dcca055
- Milestone: Milestone 1: Ratatui Setup & Lifecycle

## 🔒 Key Constraints
- Remove Rustyline REPL logic and dependency.
- Create `init_tui()` and `restore_tui()`.
- Set a panic hook that restores TUI and prints panic payload.
- Run `display_boot_sequence()` and `spawn_daemon()` before switching to raw mode.
- Minimal event loop with `q` or `Esc` to exit.
- Cargo build and test.

## Current Parent
- Conversation ID: 82f8bb16-201a-4493-bf88-99b62dcca055
- Updated: not yet

## Task Summary
- **What to build**: Ratatui TUI setup with proper terminal lifecycle and panic handling.
- **Success criteria**: compiles, tests pass, correct terminal behavior.
- **Interface contracts**: main.rs entry point.

## Key Decisions Made
- [TBD]

## Artifact Index
- [TBD]
