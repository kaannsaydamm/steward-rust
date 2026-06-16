# Handoff Report: Ratatui Setup & Lifecycle Refactor

## 1. Observation
- `crates/cli/src/tui.rs` exported `init_tui` and `restore_tui` separately, leading to potential state leaks if an error caused an early return (`?`) and `restore_tui` was bypassed.
- `crates/cli/src/main.rs` was calling `restore_tui` manually at the end of the loop and in the panic hook.
- `display_boot_sequence` in `crates/cli/src/main.rs` used `std::thread::sleep` inside an `async fn`, which blocked the tokio runtime thread.
- `spawn_daemon` hardcoded the `cargo run --bin steward-daemon` command, creating an unnecessary dependency on the build toolchain rather than using a compiled executable if available.

## 2. Logic Chain
1. To ensure the terminal state is reliably restored on program exit (including on errors), I encapsulated the `Terminal<CrosstermBackend<Stdout>>` within a `Tui` struct that implements `Drop`. The `drop` method calls `Tui::restore()`, meaning the terminal will be cleaned up safely when the `Tui` instance goes out of scope. `Deref` and `DerefMut` were also implemented to transparently pass terminal method calls.
2. Inside `main.rs`, the manual `tui::restore_tui()?` call at the end of `main` was removed. We changed initialization to `tui::Tui::init()?`.
3. In `display_boot_sequence`, the `std::thread::sleep` was replaced with `tokio::time::sleep(...).await` so that the async executor can correctly yield instead of blocking.
4. In `spawn_daemon`, I used `std::env::current_exe()` to resolve the path to the current executable. We then look for `steward-daemon` or `steward-daemon.exe` in the same directory. If either is found, we spawn it. If not, it falls back to `cargo run --bin steward-daemon`.

## 3. Caveats
- The daemon spawning fallback relies on `cargo` being in the PATH if the binary isn't adjacent to `cli`.

## 4. Conclusion
- The TUI state leak is resolved by the `Drop` implementation in the `Tui` struct.
- The blocking `std::thread::sleep` is fixed by utilizing `tokio::time::sleep`.
- The daemon spawn command now accurately tries to run the sibling executable before resorting to `cargo run`.
- Code builds and tests successfully.

## 5. Verification Method
- Code compilation was verified by running `cargo build` in `crates/cli`.
- Tests were verified by running `cargo test` in `crates/cli`.
- In addition, manual inspection of the `tui.rs` and `main.rs` source files confirms that `Tui::init` correctly returns a wrapped type which will execute `Tui::restore` upon being dropped.
