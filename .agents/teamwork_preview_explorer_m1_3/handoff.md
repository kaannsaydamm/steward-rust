# Milestone 1: Ratatui Setup & Lifecycle Analysis

This report outlines the strategy for implementing Milestone 1 in the `crates/cli` package, specifically focusing on `main.rs`.

## 1. Observation
- The `SCOPE.md` file defines Milestone 1 as: "Refactor `crates/cli/src/main.rs` to initialize `ratatui` backend and handle terminal lifecycle (crossterm raw mode, panic hook)."
- `crates/cli/Cargo.toml` already includes `ratatui = "0.30.1"` and `crossterm = "0.29.0"`.
- `crates/cli/src/main.rs` currently implements a CLI REPL using `rustyline::DefaultEditor`.
- Before the REPL starts, `main.rs` runs `display_boot_sequence()` and `spawn_daemon()` which output text and progress bars using `indicatif` to standard output.

## 2. Logic Chain
1. **Terminal Setup Timing**: Because the boot sequence and daemon spawn use `indicatif` and print directly to standard output, we must defer `ratatui` terminal initialization (entering alternate screen and enabling raw mode) until *after* these async functions complete. Initializing too early will break the progress bars and hide the boot sequence.
2. **Lifecycle Handlers**: We need `setup_terminal()` and `restore_terminal()` functions. `setup_terminal()` will call `crossterm::terminal::enable_raw_mode()` and `crossterm::execute!(io::stdout(), EnterAlternateScreen)`. `restore_terminal()` reverses this.
3. **Panic Hook**: To prevent the terminal from getting permanently stuck in raw mode if the CLI panics, a custom panic hook is required. This hook must grab the default hook, replace it with a closure that first calls `restore_terminal()`, and then invokes the original panic hook.
4. **Replacing Rustyline**: The existing `rustyline` loop (lines 85-160) must be removed. To keep the application alive and verify the lifecycle for Milestone 1, we should implement a minimal crossterm event loop (e.g., waiting for `Esc` or `q` to break the loop), which will be expanded in Milestone 2.
5. **Dependency Cleanup**: The `rustyline` import and dependency can be removed from `main.rs` and `Cargo.toml`.

## 3. Caveats
- `indicatif` could technically be removed if we decide to implement the boot sequence directly in `ratatui`, but `SCOPE.md` does not explicitly call for this, so preserving the current boot sequence text before entering the alternate screen is the safest approach.
- The `StewardServiceClient` connect call happens before the REPL loop. This should remain outside the raw mode setup or immediately after, but making it an async component within the TUI state will be handled in later milestones.

## 4. Conclusion
The implementer should perform the following changes to `crates/cli/src/main.rs`:
- Remove `rustyline` from dependencies and imports.
- Add `setup_terminal()` and `restore_terminal()` using `crossterm` and `ratatui`.
- Add `init_panic_hook()` that wraps the standard hook with a call to `restore_terminal()`.
- In `main()`:
  1. Call `display_boot_sequence().await` and `spawn_daemon().await`.
  2. Call `init_panic_hook()`.
  3. Call `setup_terminal()`.
  4. Implement a minimal crossterm `event::poll` loop that breaks when `q` or `Esc` is pressed.
  5. Call `restore_terminal()` upon normal exit.

## 5. Verification Method
1. Build the project using `cargo build -p steward-cli`.
2. Run the CLI: `cargo run -p steward-cli`.
3. Verify that the boot sequence prints normally in the standard terminal.
4. Verify that the terminal then switches to an alternate screen (blank screen or minimal UI).
5. Verify that pressing the designated exit key (e.g., `q`) gracefully exits and restores the original terminal buffer.
6. (Optional) Introduce a temporary `panic!("test panic")` inside the new loop to confirm that the panic hook correctly restores the terminal before printing the panic traceback.
