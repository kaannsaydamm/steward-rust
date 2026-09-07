//! Bridge to the vendored Prime Agent RLM substrate.
//!
//! Ported from PrimeIntellect-ai/prime-agent@844e85545af6858dcb3d6cfe42bbfcf2ca0be4e5
//! `prime-agent-runtime/src/rlm/repl.py` protocol v3 (NDJSON over stdio) — MIT.
//! Modified for Steward: the kernel speaks the sidecar contract through this
//! bridge instead of a simulated in-process executor; sessions are persistent
//! Prime REPLs keyed by session id.
//!
//! Launch: `python -m rlm.repl` with cwd pointed at the vendored runtime
//! (`vendor/prime/prime-agent-runtime/src` by default; override via
//! `STEWARD_PRIME_RLM_DIR`). Handshake: first line is
//! `{"event":"ready","protocol":3,...}`.

use anyhow::{anyhow, Context as _, Result};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Where the Prime runtime lives relative to the workspace root.
pub const PRIME_RLM_DIR_ENV: &str = "STEWARD_PRIME_RLM_DIR";
pub const PRIME_RLM_DIR_DEFAULT: &str = "vendor/prime/prime-agent-runtime/src";

fn python_executable() -> String {
    std::env::var("STEWARD_PRIME_PYTHON").unwrap_or_else(|_| "python".to_owned())
}

fn prime_rlm_dir() -> String {
    let candidate = std::env::var(PRIME_RLM_DIR_ENV).unwrap_or_else(|_| PRIME_RLM_DIR_DEFAULT.to_owned());
    // Tests run from the crate dir; resolve the workspace-root-relative
    // default against CARGO_MANIFEST_DIR when the plain path is missing.
    if std::path::Path::new(&candidate).is_dir() {
        return candidate;
    }
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(&candidate);
    workspace.to_string_lossy().into_owned()
}

struct PrimeProcess {
    child: Mutex<Child>,
    stdin: Mutex<std::process::ChildStdin>,
    events: Mutex<Vec<Value>>,
    next_id: AtomicU64,
}

impl PrimeProcess {
    /// Spawns the REPL and waits for the ready handshake. The reader pump
    /// thread holds a strong Arc for its lifetime, so the caller also gets an
    /// Arc; both share the same locks.
    fn spawn(rlm_dir: &str) -> Result<Arc<Self>> {
        let mut child = Command::new(python_executable())
            .args(["-m", "rlm.repl"])
            .current_dir(rlm_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("spawning rlm.repl from {rlm_dir}"))?;

        let stdin = child.stdin.take().context("rlm.repl stdin")?;
        let stdout = child.stdout.take().context("rlm.repl stdout")?;

        let process = Arc::new(Self {
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            events: Mutex::new(Vec::new()),
            next_id: AtomicU64::new(1),
        });

        let pump = Arc::clone(&process);
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(text) if !text.trim().is_empty() => {
                        if let Ok(value) = serde_json::from_str::<Value>(&text) {
                            pump.events.lock().push(value);
                        }
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        });

        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        loop {
            let ready = {
                let events = process.events.lock();
                events
                    .iter()
                    .any(|e| e.get("event").and_then(Value::as_str) == Some("ready"))
            };
            if ready {
                return Ok(process);
            }
            if std::time::Instant::now() > deadline {
                anyhow::bail!("rlm.repl ready handshake timed out");
            }
            let exited = {
                let mut child = process.child.lock();
                child.try_wait()?.is_some()
            };
            if exited {
                anyhow::bail!("rlm.repl exited before ready handshake");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Sends one request frame and waits for its done/result/error event.
    fn request(&self, mut frame: Value) -> Result<Value> {
        let id = format!("steward-{}", self.next_id.fetch_add(1, Ordering::Relaxed));
        frame["id"] = json!(id);
        {
            let mut stdin = self.stdin.lock();
            writeln!(stdin, "{frame}").context("writing to rlm.repl stdin")?;
            stdin.flush().ok();
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(300);
        loop {
            {
                let events = self.events.lock();
                if let Some(position) = events.iter().position(|e| {
                    e.get("id").and_then(Value::as_str) == Some(id.as_str())
                        && matches!(
                            e.get("event").and_then(Value::as_str),
                            Some("done") | Some("result") | Some("error")
                        )
                }) {
                    let event = events[position].clone();
                    drop(events);
                    self.events.lock().drain(..=position);
                    return Ok(event);
                }
            }
            if std::time::Instant::now() > deadline {
                anyhow::bail!("rlm.repl request {id} timed out");
            }
            {
                let mut child = self.child.lock();
                if child.try_wait()?.is_some() {
                    anyhow::bail!("rlm.repl exited mid-request");
                }
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn shutdown(&self) -> Result<()> {
        {
            let mut stdin = self.stdin.lock();
            let _ = writeln!(stdin, "{{\"type\":\"shutdown\"}}");
            let _ = stdin.flush();
        }
        let mut child = self.child.lock();
        let _ = child.wait();
        Ok(())
    }
}

impl Drop for PrimeProcess {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

/// Registry of live Prime REPL sessions (session id → process).
#[derive(Default)]
pub struct PrimeRlmBridge {
    sessions: Mutex<BTreeMap<String, Arc<PrimeProcess>>>,
}

impl PrimeRlmBridge {
    pub fn new() -> Self {
        Self::default()
    }

    fn process_for(&self, session_id: &str) -> Result<Arc<PrimeProcess>> {
        let mut sessions = self.sessions.lock();
        if let Some(process) = sessions.get(session_id) {
            return Ok(Arc::clone(process));
        }
        let process = PrimeProcess::spawn(&prime_rlm_dir())?;
        sessions.insert(session_id.to_owned(), Arc::clone(&process));
        Ok(process)
    }

    /// Executes one cell; returns streamed stdout plus the trailing-expression
    /// result (Prime contract: `stdout` events + `result` + `done`).
    pub fn execute_cell(
        &self,
        session_id: &str,
        _cell_id: &str,
        source: &str,
    ) -> Result<(String, Value)> {
        let process = self.process_for(session_id)?;
        let done = process.request(json!({"type": "execute", "code": source}))?;

        let status = done.get("status").and_then(Value::as_str).unwrap_or("ok");
        if status != "ok" {
            anyhow::bail!("prime cell failed: {done}");
        }
        // stdout events for this cell are consumed by the done drain above
        // (frames carry the cell id; text harvest happens on the caller side
        // via `harvest_stdout` before the drain when needed). The result value
        // rides the done frame's sibling `result` event or the `_` binding.
        let output = done
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let variables = json!({
            "result": done.get("text").cloned().unwrap_or(Value::Null),
            "session_id": session_id,
            "runtime": "prime-rlm",
        });
        Ok((output, variables))
    }

    pub fn destroy(&self, session_id: &str) -> Result<()> {
        if let Some(process) = self.sessions.lock().remove(session_id) {
            process.shutdown()?;
        }
        Ok(())
    }

    pub fn alive_sessions(&self) -> usize {
        self.sessions.lock().len()
    }
}

#[cfg(test)]
mod prime_bridge_tests {
    use super::*;

    #[test]
    fn prime_rlm_dir_contract_is_stable() {
        assert_eq!(PRIME_RLM_DIR_ENV, "STEWARD_PRIME_RLM_DIR");
        assert!(PRIME_RLM_DIR_DEFAULT.contains("prime-agent-runtime"));
    }

    #[test]
    #[ignore = "live: requires python + vendored prime runtime; run with --ignored"]
    fn live_repl_executes_and_returns_result() {
        let bridge = PrimeRlmBridge::new();
        let (output, variables) = bridge
            .execute_cell("s-test", "c1", "print('bridge')\n6*7")
            .expect("execute");
        assert!(output.contains("42"));
        assert_eq!(variables["runtime"], "prime-rlm");
        bridge.destroy("s-test").expect("destroy");
    }
}


/// Runtime selection flag: STEWARD_RLM_RUNTIME=prime activates the vendored
/// Prime substrate for all RlmRun cells.
pub fn prime_runtime_selected() -> bool {
    std::env::var("STEWARD_RLM_RUNTIME")
        .map(|v| v.eq_ignore_ascii_case("prime"))
        .unwrap_or(false)
}

/// Process-wide shared bridge (one process pool across RlmRuns).
pub fn shared_bridge() -> &'static PrimeRlmBridge {
    static BRIDGE: std::sync::OnceLock<PrimeRlmBridge> = std::sync::OnceLock::new();
    BRIDGE.get_or_init(PrimeRlmBridge::new)
}
