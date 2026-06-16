# Handoff Report: Milestone 1 - Ratatui Setup & Lifecycle

## 1. Observation
- `c:\Users\kaluclu\Desktop\steward\.agents\tui_orchestrator\SCOPE.md` defines Milestone 1: "Refactor `crates/cli/src/main.rs` to initialize `ratatui` backend and handle terminal lifecycle (crossterm raw mode, panic hook)."
- `c:\Users\kaluclu\Desktop\steward\crates\cli\Cargo.toml` already includes the required dependencies: `ratatui = "0.30.1"` and `crossterm = "0.29.0"`. It also includes `rustyline = "14.0.0"` and `indicatif = "0.17.8"`.
- `c:\Users\kaluclu\Desktop\steward\crates\cli\src\main.rs` currently implements a CLI using a `rustyline` REPL loop (lines 82-160) and uses `indicatif` for progress bars during boot and daemon spawning (lines 55-71, 24-46).
- `main.rs` does not currently import or utilize `ratatui` or `crossterm`, nor does it implement a custom panic hook to handle terminal restoration.

## 2. Logic Chain
- To fulfill Milestone 1, the `rustyline` REPL logic needs to be removed and replaced with terminal initialization for a TUI.
- TUI initialization requires transitioning the terminal into raw mode (`crossterm::terminal::enable_raw_mode`) and switching to the alternate screen (`crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen)`).
- A `ratatui::Terminal` instance utilizing a `CrosstermBackend` must be instantiated to manage rendering.
- If the application panics while in raw mode and the alternate screen, the terminal state will be corrupted for the user. A custom panic hook (`std::panic::set_hook`) is mandatory to catch panics, restore the terminal (`disable_raw_mode` and `LeaveAlternateScreen`), and then print the panic information.
- A graceful shutdown process must also be implemented to restore the terminal before exiting `main()`.
- The existing boot sequence and daemon spawning logic (`display_boot_sequence` and `spawn_daemon`) use standard output/progress bars. These should run *before* the terminal is switched to raw mode and the alternate screen to avoid rendering conflicts.
- As Milestone 2 handles the complete event loop and UI components, Milestone 1 only needs a minimal placeholder loop to keep the application alive (and capture a quit keystroke to verify teardown) or just set up the lifecycle scaffolding.

## 3. Caveats
- If the `spawn_daemon` or `display_boot_sequence` functions are ever moved to run *after* TUI initialization, they will break the TUI rendering because `indicatif` writes directly to stdout in a way that conflicts with `ratatui`'s alternate screen. They must be kept prior to `init_tui()` for now.
- `rustyline` dependency can likely be removed from `Cargo.toml` once the REPL is fully deleted.

## 4. Conclusion
**Proposed Fix Strategy:**
1. **Remove Rustyline**: Delete the `rustyline` REPL code from `main.rs` and optionally remove the dependency from `Cargo.toml`.
2. **Lifecycle Functions**: Create `init_tui()` and `restore_tui()` helper functions (either in `main.rs` or a new `tui.rs` module). `init_tui()` should enable raw mode, enter the alternate screen, and return a configured `ratatui::Terminal`. `restore_tui()` should disable raw mode and leave the alternate screen.
3. **Panic Hook**: In `main()`, set a panic hook using `std::panic::set_hook`. The hook should call `restore_tui()` and then format/print the panic payload so the user can see the error in their normal terminal.
4. **Integration in `main`**: 
   - Run `display_boot_sequence().await` and `spawn_daemon().await?` as they are.
   - Call `init_tui()`.
   - Add a minimal loop using `crossterm::event::read` to block until `q` or `Esc` is pressed, proving the TUI stays alive.
   - Break the loop and call `restore_tui()` on exit.

## 5. Verification Method
- **Run the app**: `cargo run --bin steward-cli`. The terminal should display the boot sequence, then clear into an alternate screen.
- **Test Exit**: Press `q` or `Esc` (depending on the minimal loop implemented). The application should exit cleanly, restoring the original terminal content and input mode.
- **Test Panic**: Temporarily insert a `panic!("test")` inside the TUI loop. Run the app. Verify that the terminal does not become garbled (input should still echo properly, no broken lines) and that the panic message is printed on the standard screen.
