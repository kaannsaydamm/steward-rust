# Handoff Report: Milestone 1 - Ratatui Setup & Lifecycle

## 1. Observation
- The project `steward-cli` (`crates/cli`) is currently using `rustyline` for a simple REPL, as seen in `crates/cli/src/main.rs` lines 82-160.
- `crates/cli/Cargo.toml` already includes dependencies for `ratatui = "0.30.1"` and `crossterm = "0.29.0"`. `rustyline` is also present.
- `crates/cli/src/` currently only contains `main.rs`. The `app.rs` and `ui.rs` files mentioned in `SCOPE.md` have not been created yet.
- The `main.rs` file has preliminary setup functions `display_boot_sequence()` and `spawn_daemon()` which output to `stdout` using `indicatif` progress bars before starting the REPL.

## 2. Logic Chain
- To achieve **Milestone 1**, we must replace the `rustyline` REPL with the `ratatui` terminal lifecycle.
- Proper `ratatui` setup using `crossterm` requires modifying terminal state (entering alternate screen, enabling raw mode) and restoring it upon exit.
- A **panic hook** is crucial. If the application panics while the terminal is in raw mode and the alternate screen, the terminal will become unusable (corrupted output). The panic hook must catch panics, restore the terminal state to normal, and then print the panic message.
- Since `app.rs` and `ui.rs` (UI components & Event Loop) are scheduled for **Milestone 2**, the implementation for Milestone 1 should focus purely on initialization, a simple mechanism to keep the application running (e.g., a basic loop waiting for `'q'` to quit), and a clean teardown.
- The initial boot sequence (`display_boot_sequence` and `spawn_daemon`) can remain as standard `stdout` printing since they execute *before* `ratatui` takes over the terminal.

## 3. Caveats
- Moving to `ratatui` means the REPL commands (`status`, `task <desc>`) will temporarily be unavailable until the input field and event loop are implemented in Milestone 2.
- The placeholder event loop in M1 will block standard execution until M2 integrates a proper asynchronous event loop (e.g., using `tokio::select!` for TUI events and gRPC responses).

## 4. Conclusion
**Recommended Fix Strategy for `crates/cli/src/main.rs`:**
1. **Remove `rustyline`**: Delete REPL code and the `rustyline` crate dependency in `Cargo.toml` (optional, but good practice).
2. **Terminal Initialization**: Create a function `init_tui() -> Result<Terminal<CrosstermBackend<std::io::Stdout>>>` that calls `enable_raw_mode()` and `execute!(stdout(), EnterAlternateScreen)`.
3. **Terminal Restoration**: Create a function `restore_tui() -> Result<()>` that calls `disable_raw_mode()` and `execute!(stdout(), LeaveAlternateScreen)`.
4. **Panic Hook**: In `main()`, capture the original panic hook via `std::panic::take_hook()`. Set a new hook with `std::panic::set_hook()` that calls `restore_tui()` and then calls the original hook.
5. **Main Lifecycle**:
   - Run existing boot sequence & daemon setup.
   - Initialize the gRPC client.
   - Call `init_tui()`.
   - Implement a temporary loop waiting for `crossterm::event::read()` to return a `'q'` keypress (this proves crossterm works and holds the program open).
   - Exit the loop and call `restore_tui()`.

## 5. Verification Method
- **Implementation check**: The implementer will modify `crates/cli/src/main.rs` based on the strategy.
- **Run the code**: Execute `cargo run --bin steward-cli`. The terminal should clear (alternate screen), wait for input, and return cleanly when `q` is pressed.
- **Panic check**: Temporarily add a `panic!("test")` inside the loop. Run the CLI and ensure the terminal is cleanly restored and the panic stack trace is visible.
- **Tests**: Run `cargo test -p steward-cli` to ensure no syntax errors or breaking changes.
