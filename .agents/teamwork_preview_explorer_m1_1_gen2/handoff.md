# Handoff Report: Ratatui Setup & Lifecycle Refactor

## 1. Observation
- In `crates/cli/src/main.rs:77`, `main()` returns `Result<()>`. If an error occurs (e.g., `terminal.draw` or `event::poll` fails), the `?` operator will cause an early return, skipping `tui::restore_tui()?` at the end of the function (line 114).
- The panic hook in `main.rs:79` calls `tui::restore_tui()` correctly on panic, but this does not cover `Result::Err` returns.
- In `crates/cli/src/tui.rs:9-15`, `init_tui()` executes `EnterAlternateScreen` and `enable_raw_mode()` *before* calling `Terminal::new()`. If `Terminal::new()` fails and returns `Err`, the function exits early, leaving the terminal in raw mode and alternate screen without cleanup.
- In `crates/cli/src/main.rs:59-69`, `display_boot_sequence()` is an `async fn` but uses `std::thread::sleep(Duration::from_millis(5))` (line 67). This blocks the async executor thread.
- In `crates/cli/src/main.rs:32-41`, `spawn_daemon()` uses `tokio::process::Command::new("cargo").arg("run").arg("--bin").arg("steward-daemon")`. This is problematic for a compiled release binary, as it relies on `cargo` being present and re-compiling/running from source.

## 2. Logic Chain
1.  **TUI State Leak on Error:** Because Rust's `?` operator performs early returns on error, any failure within the TUI event loop will skip the explicit `tui::restore_tui()` call at the end of `main`. To guarantee cleanup regardless of how the scope is exited (normal return, error return, or panic), the cleanup logic should be tied to the `Drop` trait of a structure that lives as long as the TUI state, or by using a dedicated closure/wrapper that ensures execution on exit. Given `ratatui`'s common patterns, creating a wrapper struct for the TUI that implements `Drop` is the most robust approach.
2.  **Partial Initialization Leak:** If an error happens *during* initialization (e.g., after `enable_raw_mode()` but before `Terminal::new()` succeeds), the partial state must be cleaned up. Creating a struct that manages setup in its constructor and cleanup in its `Drop` implementation solves both the partial initialization leak and the TUI state leak on error. When `init_tui()` fails midway, the partially constructed resources shouldn't leave global side effects, or a robust setup function should catch internal errors, attempt cleanup, and then return the error.
3.  **Blocking Async:** Using `std::thread::sleep` inside `async fn display_boot_sequence()` blocks the underlying tokio worker thread. It should be replaced with `tokio::time::sleep(...).await` to yield control back to the executor.
4.  **Daemon Spawn Issue:** Relying on `cargo run` inside `spawn_daemon` assumes a development environment. For a production CLI tool, it should execute the `steward-daemon` binary directly (e.g., resolving its path relative to the current executable `std::env::current_exe()`).

## 3. Caveats
- I am recommending a struct-based approach implementing `Drop` for TUI management. Another valid approach is using the `scopeguard` crate, but a custom `Drop` struct keeps the dependency footprint small and is idiomatic for TUI wrappers.
- For `spawn_daemon`, it assumes `steward-daemon` is distributed alongside the CLI executable. The implementer will need to verify the intended deployment model. If it's pure development for now, it might be deferred, but it's flagged as an issue.
- The `spawn_daemon` check looks for "steward-daemon" or "steward-daemon.exe" in `sysinfo`. This is a bit brittle but acceptable for a milestone 1 implementation.

## 4. Conclusion
To fix the listed issues, the following strategy should be implemented:

1.  **TUI Lifecycle Management:** Replace `init_tui` and `restore_tui` with a struct (e.g., `struct Tui`) that manages the terminal lifecycle.
    *   In its `new` (or `init`) method: Enable raw mode, enter alternate screen, and initialize `ratatui::Terminal`. If any step fails, catch the error, attempt to restore the terminal state (disable raw mode, leave alt screen), and return the error.
    *   Implement `Drop` for this struct: In `drop`, call the restoration logic (`disable_raw_mode`, `LeaveAlternateScreen`). This guarantees cleanup on successful exit, error return, and panics (though keeping a panic hook that safely invokes restoration is still good practice).
2.  **Fix Async Sleep:** In `main.rs`, update `display_boot_sequence` to use `tokio::time::sleep(Duration::from_millis(5)).await` instead of `std::thread::sleep`.
3.  **Fix Daemon Spawning:** Modify `spawn_daemon` to locate and execute the compiled `steward-daemon` executable relative to `std::env::current_exe()`, rather than using `cargo run`.

## 5. Verification Method
- **Code Inspection:** Check that `tui.rs` defines a struct implementing `Drop` and that `main.rs` holds an instance of it.
- **Test TUI Error:** Temporarily insert an `Err(anyhow::anyhow!("test error"))` inside the TUI event loop in `main.rs`. Run the CLI. Observe that the terminal correctly returns to its original state (not raw mode, not stuck in alternate screen) when the program exits with the error.
- **Test Partial Init Error:** Temporarily mock `Terminal::new` to fail in `init_tui`. Run the CLI. Ensure the terminal is restored correctly.
- **Verify Async Sleep:** Ensure `std::thread::sleep` is removed from `display_boot_sequence` and `tokio::time::sleep` is used.
- **Verify Daemon Spawn:** Run the compiled binary (e.g., via `cargo build --release` then executing the binary directly) and ensure it attempts to spawn the daemon correctly without relying on `cargo`.
