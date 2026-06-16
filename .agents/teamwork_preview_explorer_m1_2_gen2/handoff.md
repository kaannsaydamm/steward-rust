# Ratatui Setup & Lifecycle Analysis

## 1. Observation

Based on the prompt and the source files (`crates/cli/src/main.rs`, `crates/cli/src/tui.rs`), the following issues were reported:

*   **TUI State Leak on Error (Critical):** If an error occurs inside the TUI event loop in `main.rs` (e.g., `terminal.draw` or `event::poll`), `main()` returns an `Err` immediately due to the `?` operator. The panic hook only intercepts `panic!`, not `Result::Err`. The execution bypasses `tui::restore_tui()`, leaving the terminal in raw mode and stuck in the alternate screen.
*   **Partial Initialization Leak:** In `tui.rs`, `init_tui()` does this:
    ```rust
    pub fn init_tui() -> Result<Terminal<CrosstermBackend<Stdout>>> {
        stdout().execute(EnterAlternateScreen)?;
        enable_raw_mode()?;
        let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
        terminal.clear()?;
        Ok(terminal)
    }
    ```
    If `Terminal::new(...)` or `terminal.clear()?` fails, the function returns early with an `Err`. `disable_raw_mode()` and `LeaveAlternateScreen` are not called.
*   **Blocking Async:** In `main.rs`, `display_boot_sequence()` is an `async fn` but it uses `std::thread::sleep(Duration::from_millis(5))` instead of `tokio::time::sleep()`. This blocks the current tokio thread, preventing other async tasks on that thread from running.
*   **Daemon Spawn Issue:** In `main.rs`, `spawn_daemon()` uses `cargo run --bin steward-daemon` to spawn the daemon. This assumes the presence of `cargo` and the source code at runtime, which is not true for a distributed/compiled release binary.

## 2. Logic Chain

*   **Fixing TUI State Leak:** The standard way to guarantee cleanup in Rust, regardless of whether a function returns `Ok`, `Err`, or panics, is using the RAII pattern (Resource Acquisition Is Initialization) via the `Drop` trait. We should introduce a struct (e.g., `Tui`) that initializes the terminal state upon creation and restores it in its `Drop` implementation. Alternatively, we can use the `scopeguard` crate or write a custom `TuiGuard` struct.

    Let's design a custom RAII struct `TuiGuard` in `tui.rs` or `main.rs`:
    ```rust
    pub struct TuiGuard {
        pub terminal: Terminal<CrosstermBackend<Stdout>>,
    }

    impl TuiGuard {
        pub fn init() -> Result<Self> {
            stdout().execute(EnterAlternateScreen)?;
            enable_raw_mode()?;
            // If Terminal::new fails, we must clean up what we did above.
            let terminal = match Terminal::new(CrosstermBackend::new(stdout())) {
                Ok(mut t) => {
                    if let Err(e) = t.clear() {
                        let _ = disable_raw_mode();
                        let _ = stdout().execute(LeaveAlternateScreen);
                        return Err(e.into());
                    }
                    t
                }
                Err(e) => {
                    let _ = disable_raw_mode();
                    let _ = stdout().execute(LeaveAlternateScreen);
                    return Err(e.into());
                }
            };
            Ok(Self { terminal })
        }
    }

    impl Drop for TuiGuard {
        fn drop(&mut self) {
            let _ = disable_raw_mode();
            let _ = stdout().execute(LeaveAlternateScreen);
        }
    }
    ```
    Using `TuiGuard` solves both the "TUI State Leak on Error" and the "Partial Initialization Leak". It completely replaces the manual `init_tui` and `restore_tui` function calls in `main.rs`.

*   **Fixing Blocking Async:** In `display_boot_sequence`, change `std::thread::sleep(Duration::from_millis(5));` to `tokio::time::sleep(Duration::from_millis(5)).await;`.

*   **Fixing Daemon Spawn Issue:** Using `cargo run` is not viable for a compiled binary. We should look for the compiled daemon binary in the same directory as the current executable. We can use `std::env::current_exe()` to get the current executable's path, then change the filename to `steward-daemon` (or `steward-daemon.exe` on Windows).

    ```rust
    let mut daemon_path = std::env::current_exe().context("Failed to get current exe path")?;
    daemon_path.set_file_name(if cfg!(windows) { "steward-daemon.exe" } else { "steward-daemon" });

    let _child = tokio::process::Command::new(daemon_path)
        // ...
    ```
    Fallback logic can be added: if `steward-daemon` binary is not found next to the CLI, it could optionally try `cargo run` in development, but looking for the adjacent executable is the primary correct approach for a released binary.

## 3. Caveats

*   I cannot make code modifications, only provide recommendations.
*   When fixing `spawn_daemon`, it assumes `steward-daemon` is distributed in the same directory as the CLI executable. If it's located elsewhere, the path resolution logic must be adjusted.
*   The `Drop` trait implementation suppresses cleanup errors (using `let _ = ...`). This is standard practice in `Drop` implementations to prevent double panics, but it means if restoring the terminal state fails, it will fail silently.

## 4. Conclusion

1.  **Replace manual init/restore with RAII `Drop`:** Create a wrapper struct for the `Terminal` (e.g., `Tui`) that initializes raw mode and alternate screen in a constructor, handles errors during partial initialization by cleaning up before returning `Err`, and implements `Drop` to automatically call `disable_raw_mode()` and `LeaveAlternateScreen()`. This ensures cleanup happens on both `Err` returns and panics.
2.  **Fix Blocking Async:** Replace `std::thread::sleep` with `tokio::time::sleep(..).await` in `display_boot_sequence()`.
3.  **Fix Daemon Spawning:** Modify `spawn_daemon()` to locate the `steward-daemon` executable relative to the current executable's path (`std::env::current_exe()`) instead of hardcoding `cargo run`.

## 5. Verification Method

*   **Code Review:** Ensure `Tui` wrapper with `Drop` is used in `main.rs`. Verify `std::thread::sleep` is removed from `async` functions.
*   **Test TUI State Leak:** Modify the event loop in `main.rs` to intentionally return `Err(anyhow::anyhow!("test error"))` when a specific key (e.g., 'e') is pressed. Run the CLI and press 'e'. The terminal should cleanly exit raw mode and the alternate screen, printing the error message normally.
*   **Test Daemon Spawning:** Build both binaries using `cargo build --release`. Run the released CLI executable directly without `cargo run`. It should successfully locate and spawn the release daemon binary next to it.
