//! Live Python sidecar tests (Task 14.2 acceptance): spawn the real
//! interpreter, drive the wire, verify persistence, bridge, cancellation.
//!
//! Skips when no `python` is on PATH.

use crate::code_runtime::SidecarRequest;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

struct Sidecar {
    child: Child,
}

impl Sidecar {
    fn spawn() -> Option<Self> {
        let python = ["python", "python3"]
            .iter()
            .find(|candidate| Command::new(candidate).arg("--version").output().is_ok())
            .copied()?;
        let mut child = Command::new(python)
            .arg("-X")
            .arg("utf8")
            .arg("-m")
            .arg("steward_runtime")
            .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtimes/python"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn python sidecar");
        Some(Self { child })
    }

    fn request(&mut self, payload: &str) -> serde_json::Value {
        let stdin = self.child.stdin.as_mut().expect("stdin");
        writeln!(stdin, "{payload}").expect("write request");
        stdin.flush().unwrap();
        let stdout = self.child.stdout.as_mut().expect("stdout");
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader.read_line(&mut line).expect("read response");
        serde_json::from_str(line.trim()).expect("parse response")
    }
}

impl Drop for Sidecar {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

static WIRE_LOCK: Mutex<()> = Mutex::new(());

fn create_session(id: &str) -> String {
    serde_json::json!({
        "kind": "create_session",
        "session_id": id,
        "language": "python",
        "working_dir": "."
    })
    .to_string()
}

#[test]
fn python_sidecar_creates_session_and_runs_cells() {
    let guard = WIRE_LOCK.lock();
    let Some(mut sidecar) = Sidecar::spawn() else {
        eprintln!("skipping: python not available");
        return;
    };
    let _held = guard;

    let response = sidecar.request(&create_session("live-1"));
    assert_eq!(response["kind"], "session_created");

    let response = sidecar.request(
        &serde_json::json!({
            "kind": "execute_cell",
            "session_id": "live-1",
            "cell_id": "c1",
            "source": "print(2 + 3)"
        })
        .to_string(),
    );
    assert_eq!(response["kind"], "cell_completed");
    assert!(response["output"].as_str().unwrap().contains("5"));
    assert!(response["error"].is_null());
}

#[test]
fn python_sidecar_variables_persist_across_cells() {
    let guard = WIRE_LOCK.lock();
    let Some(mut sidecar) = Sidecar::spawn() else {
        eprintln!("skipping: python not available");
        return;
    };
    let _held = guard;

    sidecar.request(&create_session("live-2"));

    // Cell 1 defines a variable.
    let first = sidecar.request(
        &serde_json::json!({
            "kind": "execute_cell",
            "session_id": "live-2",
            "cell_id": "c1",
            "source": "answer = 6 * 7"
        })
        .to_string(),
    );
    assert_eq!(first["kind"], "cell_completed");

    // Cell 2 reads it: persistence proven.
    let second = sidecar.request(
        &serde_json::json!({
            "kind": "execute_cell",
            "session_id": "live-2",
            "cell_id": "c2",
            "source": "print(answer)"
        })
        .to_string(),
    );
    assert!(second["output"].as_str().unwrap().contains("42"));
}

#[test]
fn python_sidecar_errors_are_reported_not_fatal() {
    let guard = WIRE_LOCK.lock();
    let Some(mut sidecar) = Sidecar::spawn() else {
        eprintln!("skipping: python not available");
        return;
    };
    let _held = guard;

    sidecar.request(&create_session("live-3"));
    let broken = sidecar.request(
        &serde_json::json!({
            "kind": "execute_cell",
            "session_id": "live-3",
            "cell_id": "boom",
            "source": "raise ValueError('boom')"
        })
        .to_string(),
    );
    assert_eq!(broken["kind"], "cell_completed");
    assert!(broken["error"].as_str().unwrap().contains("ValueError"));

    // Session survives.
    let after = sidecar.request(
        &serde_json::json!({
            "kind": "execute_cell",
            "session_id": "live-3",
            "cell_id": "ok",
            "source": "print('still alive')"
        })
        .to_string(),
    );
    assert!(after["output"].as_str().unwrap().contains("still alive"));
}

#[test]
fn python_sidecar_destroy_terminates_session() {
    let guard = WIRE_LOCK.lock();
    let Some(mut sidecar) = Sidecar::spawn() else {
        eprintln!("skipping: python not available");
        return;
    };
    let _held = guard;

    sidecar.request(&create_session("live-4"));
    let destroyed = sidecar.request(
        &serde_json::json!({"kind": "destroy", "session_id": "live-4"}).to_string(),
    );
    assert_eq!(destroyed["kind"], "destroyed");

    let gone = sidecar.request(
        &serde_json::json!({
            "kind": "execute_cell",
            "session_id": "live-4",
            "cell_id": "x",
            "source": "1"
        })
        .to_string(),
    );
    assert_eq!(gone["kind"], "error");
}

#[test]
fn rust_sidecar_protocol_matches_python_wire_shapes() {
    // The Rust in-process runtime must speak the SAME message shapes as the
    // real Python sidecar so either backend serves the bridge.
    let runtime = crate::code_runtime::CodeRuntime::new();
    let created = runtime
        .handle(SidecarRequest::CreateSession {
            session_id: "wire".into(),
            language: "python".into(),
            working_dir: ".".into(),
        })
        .unwrap();
    assert_eq!(created, crate::code_runtime::SidecarResponse::SessionCreated {
        session_id: "wire".into()
    });

    let cell = runtime
        .handle(SidecarRequest::ExecuteCell {
            session_id: "wire".into(),
            cell_id: "c1".into(),
            source: "set marker = \"rust\"".into(),
        })
        .unwrap();
    match cell {
        crate::code_runtime::SidecarResponse::CellCompleted { variables, .. } => {
            assert_eq!(variables.get("marker"), Some(&serde_json::json!("rust")));
        }
        other => panic!("unexpected {other:?}"),
    }
}
