# Handoff Report: Milestone 1 Review

## 1. Observation
I reviewed `crates/cli/Cargo.toml`, `crates/cli/src/main.rs`, and `crates/cli/src/tui.rs`.
- `main.rs` contains a panic hook that calls `tui::restore_tui()` correctly.
- `main.rs` contains an event loop for the TUI:
```rust
    let mut terminal = tui::init_tui()?;

    loop {
        terminal.draw(|f| { ... })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? { ... }
        }
    }

    tui::restore_tui()?;
```
- `tui::init_tui()` contains the following sequence:
```rust
pub fn init_tui() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;
    Ok(terminal)
}
```
- The daemon spawn function in `main.rs` uses: `tokio::process::Command::new("cargo").arg("run").arg("--bin").arg("steward-daemon")`.

## 2. Logic Chain
1. **Terminal state corruption on loop errors**: In `main.rs`, the TUI event loop uses the `?` operator. If `terminal.draw`, `event::poll`, or `event::read` returns an `Err`, the `main` function will immediately return `Result::Err`. The rust panic hook is *not* invoked for `Result::Err` returns, and the `tui::restore_tui()?` at the end of the file is skipped. Consequently, the user's terminal is left in raw mode and on the alternate screen, requiring a hard reset.
2. **Terminal state corruption on initialization errors**: In `init_tui()`, if `Terminal::new` or `terminal.clear` fails *after* `enable_raw_mode()` has succeeded, the function returns `Err` via `?`, bypassing `disable_raw_mode()`. This again leaves the terminal broken.
3. **Hardcoded cargo dependency**: The CLI relies on `cargo run --bin steward-daemon` to spawn the background daemon. This means the CLI is not a self-contained production binary and relies on the user running it from the source tree with the Rust toolchain installed.

## 3. Caveats
- Due to file locks on the `cargo` build directory (likely held by other background tasks or analyzer processes), I could not complete a clean `cargo build` run to test the runtime behavior physically. However, the static logic flaws in the TUI lifecycle are objective and guaranteed to cause state leaks on error paths.

## 4. Conclusion
**Verdict: REQUEST_CHANGES** (FAIL)
The terminal lifecycle implementation lacks basic error-path safety. The current implementation only restores terminal state on the "happy path" (user presses 'q' or 'Esc') or on a `panic!`. Normal IO errors propagated via `?` will break the user's terminal. Furthermore, spawning the daemon via `cargo run` is a dirty hack that shouldn't make it to production unless explicitly accepted as a dev-only shortcut.

**Recommendations for Implementer**:
1. Wrap the TUI initialization and execution loop in a separate function. Call `restore_tui()` in a `defer!` block or use a custom struct that implements the `Drop` trait to ensure `disable_raw_mode` is *always* called, regardless of how the function exits (Ok, Err, or panic).
2. Rethink the `spawn_daemon` logic to execute the actual compiled `steward-daemon` binary relative to the CLI executable's path, rather than using `cargo run`.

## 5. Verification Method
1. Inspect `crates/cli/src/main.rs`. Mentally trace the execution path if `event::poll()` returns an `io::Error`. Note that `main()` returns early and skips `tui::restore_tui()`.
2. Inspect `crates/cli/src/tui.rs`. Mentally trace the execution path if `Terminal::new()` returns an `io::Error` after `enable_raw_mode()` succeeds. Note that raw mode is leaked.
