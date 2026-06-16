# Observation
1. **TUI State Leak on Error / Partial Initialization Leak:**
   - In `crates/cli/src/main.rs`, the TUI event loop uses `?` for error propagation: `terminal.draw(...)?;` and `event::poll(...)?;`.
   - If an error occurs, `main` returns `Err` immediately, skipping `tui::restore_tui()?;` at line 115.
   - In `crates/cli/src/tui.rs`, `init_tui()` calls `stdout().execute(EnterAlternateScreen)?;` and `enable_raw_mode()?;` before `Terminal::new(...)`. If `Terminal::new` fails, it returns an error, leaving the terminal in the alternate screen and raw mode.
   - The panic hook in `main.rs:79` calls `tui::restore_tui()`, but this only handles `panic!`, not `Result::Err`.
2. **Blocking Async:**
   - `crates/cli/src/main.rs:67` inside `async fn display_boot_sequence()` uses `std::thread::sleep(Duration::from_millis(5));` which blocks the current thread, starving the Tokio async runtime.
3. **Daemon Spawn Issue:**
   - `crates/cli/src/main.rs:32-35` hardcodes `cargo run --bin steward-daemon`. This will fail in a release build or when the CLI is run outside a Cargo workspace.

# Logic Chain
1. **TUI Restoration:** To guarantee that the terminal is restored regardless of whether `main` exits via `Ok`, `Err`, or `panic!`, we must use RAII (Resource Acquisition Is Initialization). We can create a struct (e.g., `Tui`) that initializes the terminal state on creation and implements the `Drop` trait to call `restore_tui()` when it goes out of scope. This prevents both partial initialization leaks and early-return leaks.
2. **Blocking Async Fix:** Async functions must not block the OS thread. Replacing `std::thread::sleep` with `tokio::time::sleep(...).await` will yield execution back to the Tokio scheduler.
3. **Daemon Path Resolution:** Hardcoding `cargo run` assumes a development environment. We need a robust way to find the daemon executable. A common strategy is to assume the daemon executable is in the same directory as the CLI executable. We can use `std::env::current_exe()` to get the CLI path, change the filename to `steward-daemon` (with `.exe` extension on Windows), and attempt to spawn that. If it fails or doesn't exist, we can fallback to `cargo run --bin steward-daemon` for development ergonomics.

# Caveats
- Relying entirely on `Drop` means we don't return `Result` from the cleanup, so any errors during `LeaveAlternateScreen` or `disable_raw_mode` in the `Drop` implementation will be silently ignored (which is standard practice for cleanup code).
- The daemon executable path resolution assumes `steward-daemon` is located alongside the `steward-cli` binary.
- Windows executable extensions (`.exe`) must be handled if constructing paths manually, though on Windows, `Command::new("steward-daemon")` usually resolves `.exe` automatically if it's in the PATH. It's safer to use `env::consts::EXE_EXTENSION`.

# Conclusion
The `main.rs` and `tui.rs` files need to be refactored to:
1. Implement a `Tui` struct in `tui.rs` that encapsulates `Terminal` and implements `Drop` for reliable cleanup (calling `disable_raw_mode` and `LeaveAlternateScreen`).
2. Update `main.rs` to use this new `Tui` struct instead of manually calling `init_tui` and `restore_tui`. The panic hook should also be updated or can potentially be removed if the `Drop` trait handles panics effectively (though a panic hook is still useful to ensure cleanup happens *before* panic info is printed, so the hook should remain but rely on a robust cleanup mechanism).
3. Change `std::thread::sleep(Duration::from_millis(5))` to `tokio::time::sleep(Duration::from_millis(5)).await` in `display_boot_sequence()`.
4. Modify `spawn_daemon()` to try running the `steward-daemon` binary relative to `std::env::current_exe()`, and fallback to `cargo run` if needed.

# Verification Method
1. Introduce an artificial error inside the `loop` in `main.rs` (e.g., `Err(anyhow::anyhow!("Test error"))?`). Run the application. The terminal should not be left in raw mode or alternate screen.
2. Verify `cargo run` builds and executes correctly.
3. Verify the boot sequence spinner feels responsive and `cargo run` shows no blocking warnings.
4. Compile the project with `cargo build --release` and run the CLI directly from `target/release/steward-cli` to verify it successfully spawns `steward-daemon` without relying on `cargo run`.
