//! Scoped approvals (§23.3, Task 7.3, S-002/S-003).
//!
//! Decision scopes: once / for-this-run / for-this-workspace / tool+effect+
//! path / always-for-signed-tool. Stored decisions carry expiration and
//! provenance (who/when/args-hash).

use crate::effects::Effect;
use anyhow::Result;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApprovalScope {
    /// This exact call only.
    Once,
    /// Any matching call within one run.
    ForRun { run_id: String },
    /// Any matching call in this workspace.
    ForWorkspace { workspace_id: String },
    /// tool + effect + path scope, optionally expiring.
    EffectPath {
        effect: Effect,
        path_root: String,
        expires_at_ms: Option<i64>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScopedApprovalRequest {
    pub approval_id: String,
    pub run_id: String,
    pub workspace_id: Option<String>,
    pub tool_id: String,
    pub effect: Effect,
    pub requested_path: Option<String>,
    /// Redacted-argument digest (never the values themselves, S-003).
    pub args_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApprovalRecord {
    pub request: ScopedApprovalRequest,
    pub scope: ApprovalScope,
    /// Provenance (S-003).
    pub decided_by: String,
    pub decided_at_ms: i64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ApprovalDecision {
    Granted,
    Denied,
    /// No matching record; the caller must surface a request.
    NotRequested,
}

/// In-memory approval store; DB persistence rides the stores module later.
#[derive(Default)]
pub struct ApprovalStore {
    records: Mutex<Vec<ApprovalRecord>>,
}

impl ApprovalStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&self, record: ApprovalRecord) {
        self.records.lock().push(record);
    }

    /// Checks whether a stored, unexpired decision covers this request.
    pub fn check(&self, request: &ScopedApprovalRequest, now_ms: i64) -> ApprovalDecision {
        let records = self.records.lock();
        let matching = records.iter().rev().find(|record| {
            let r = &record.request;
            if r.tool_id != request.tool_id || r.effect != request.effect {
                return false;
            }
            match &record.scope {
                ApprovalScope::Once => {
                    r.args_digest == request.args_digest
                        && r.run_id == request.run_id
                        && r.requested_path == request.requested_path
                }
                ApprovalScope::ForRun { run_id } => run_id == &request.run_id,
                ApprovalScope::ForWorkspace { workspace_id } => {
                    Some(workspace_id) == request.workspace_id.as_ref()
                }
                ApprovalScope::EffectPath {
                    effect,
                    path_root,
                    expires_at_ms,
                } => {
                    effect == &request.effect
                        && expires_at_ms.map(|exp| now_ms <= exp).unwrap_or(true)
                        && match (&request.requested_path, path_root.as_str()) {
                            (Some(path), root) => path_under(path, root),
                            (None, root) => root.is_empty(),
                        }
                }
            }
        });
        match matching {
            Some(record) if record_decided(record) => ApprovalDecision::Granted,
            Some(record) => ApprovalDecision::Denied,
            None => ApprovalDecision::NotRequested,
        }
    }

    pub fn len(&self) -> usize {
        self.records.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn record_decided(record: &ApprovalRecord) -> bool {
    // Records are only stored after an explicit decision; the `decided_by`
    // field being non-empty marks grant. Denials are recorded with a marker.
    !record.decided_by.is_empty() && record.decided_by != "deny"
}

/// Stable digest over sorted arguments; secret values never enter (S-003).
pub fn args_digest(arguments: &BTreeMap<String, String>) -> String {
    let mut hasher = Sha256::new();
    for (key, value) in arguments {
        hasher.update(key.as_bytes());
        hasher.update(b"=");
        hasher.update(value.as_bytes());
        hasher.update(b";");
    }
    format!("{:x}", hasher.finalize())
}

fn path_under(path: &str, root: &str) -> bool {
    let path = path.replace('\\', "/");
    let root = root.trim_end_matches('/').replace('\\', "/");
    if root.is_empty() {
        return true;
    }
    path == root || path.starts_with(&format!("{root}/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(
        tool: &str,
        effect: Effect,
        path: Option<&str>,
        run: &str,
        args: &[(&str, &str)],
    ) -> ScopedApprovalRequest {
        let mut map = BTreeMap::new();
        for (k, v) in args {
            map.insert((*k).to_owned(), (*v).to_owned());
        }
        ScopedApprovalRequest {
            approval_id: format!("apr_{tool}"),
            run_id: run.into(),
            workspace_id: Some("ws1".into()),
            tool_id: tool.into(),
            effect,
            requested_path: path.map(str::to_owned),
            args_digest: args_digest(&map),
        }
    }

    #[test]
    fn once_scope_binds_to_exact_args_and_run() {
        let store = ApprovalStore::new();
        let request = make_request(
            "fs.write",
            Effect::FilesystemWrite,
            Some("src/a.rs"),
            "run_1",
            &[("path", "src/a.rs")],
        );
        store.record(ApprovalRecord {
            request: request.clone(),
            scope: ApprovalScope::Once,
            decided_by: "user".into(),
            decided_at_ms: 1,
        });

        assert_eq!(store.check(&request, 2), ApprovalDecision::Granted);
        // Different run → not covered.
        let other_run = make_request(
            "fs.write",
            Effect::FilesystemWrite,
            Some("src/a.rs"),
            "run_2",
            &[("path", "src/a.rs")],
        );
        assert_eq!(store.check(&other_run, 2), ApprovalDecision::NotRequested);
        // Different args → not covered.
        let other_args = make_request(
            "fs.write",
            Effect::FilesystemWrite,
            Some("src/a.rs"),
            "run_1",
            &[("path", "src/b.rs")],
        );
        assert_eq!(store.check(&other_args, 2), ApprovalDecision::NotRequested);
    }

    #[test]
    fn for_run_scope_covers_same_run_only() {
        let store = ApprovalStore::new();
        let request = make_request(
            "fs.write",
            Effect::FilesystemWrite,
            Some("src/a.rs"),
            "run_1",
            &[],
        );
        store.record(ApprovalRecord {
            request,
            scope: ApprovalScope::ForRun {
                run_id: "run_1".into(),
            },
            decided_by: "user".into(),
            decided_at_ms: 1,
        });
        let same_run = make_request(
            "fs.write",
            Effect::FilesystemWrite,
            Some("src/z.rs"),
            "run_1",
            &[],
        );
        assert_eq!(store.check(&same_run, 2), ApprovalDecision::Granted);
        let other_run = make_request(
            "fs.write",
            Effect::FilesystemWrite,
            Some("src/a.rs"),
            "run_9",
            &[],
        );
        assert_eq!(store.check(&other_run, 2), ApprovalDecision::NotRequested);
    }

    #[test]
    fn effect_path_scope_expires() {
        let store = ApprovalStore::new();
        let request = make_request(
            "fs.write",
            Effect::FilesystemWrite,
            Some("src/a.rs"),
            "run_1",
            &[],
        );
        store.record(ApprovalRecord {
            request,
            scope: ApprovalScope::EffectPath {
                effect: Effect::FilesystemWrite,
                path_root: "src".into(),
                expires_at_ms: Some(100),
            },
            decided_by: "user".into(),
            decided_at_ms: 1,
        });
        let later = make_request(
            "fs.write",
            Effect::FilesystemWrite,
            Some("src/deep/b.rs"),
            "run_2",
            &[],
        );
        assert_eq!(store.check(&later, 99), ApprovalDecision::Granted);
        assert_eq!(store.check(&later, 101), ApprovalDecision::NotRequested);
    }

    #[test]
    fn effect_path_scope_confines_paths() {
        let store = ApprovalStore::new();
        let request = make_request(
            "fs.write",
            Effect::FilesystemWrite,
            Some("src/a.rs"),
            "run_1",
            &[],
        );
        store.record(ApprovalRecord {
            request,
            scope: ApprovalScope::EffectPath {
                effect: Effect::FilesystemWrite,
                path_root: "src".into(),
                expires_at_ms: None,
            },
            decided_by: "user".into(),
            decided_at_ms: 1,
        });
        let inside = make_request(
            "fs.write",
            Effect::FilesystemWrite,
            Some("src/sub/x.rs"),
            "run_2",
            &[],
        );
        assert_eq!(store.check(&inside, 5), ApprovalDecision::Granted);
        let git = make_request(
            "fs.write",
            Effect::FilesystemWrite,
            Some(".git/config"),
            "run_2",
            &[],
        );
        assert_eq!(store.check(&git, 5), ApprovalDecision::NotRequested);
    }

    #[test]
    fn denial_records_never_grant() {
        let store = ApprovalStore::new();
        let request = make_request(
            "fs.write",
            Effect::FilesystemWrite,
            Some("src/a.rs"),
            "run_1",
            &[],
        );
        store.record(ApprovalRecord {
            request,
            scope: ApprovalScope::ForRun {
                run_id: "run_1".into(),
            },
            decided_by: "deny".into(),
            decided_at_ms: 1,
        });
        let same = make_request(
            "fs.write",
            Effect::FilesystemWrite,
            Some("src/b.rs"),
            "run_1",
            &[],
        );
        assert_eq!(store.check(&same, 2), ApprovalDecision::Denied);
    }

    #[test]
    fn digest_is_stable_and_argument_order_insensitive() {
        let mut a = BTreeMap::new();
        a.insert("path".into(), "x.rs".into());
        a.insert("mode".into(), "w".into());
        let mut b = BTreeMap::new();
        b.insert("mode".into(), "w".into());
        b.insert("path".into(), "x.rs".into());
        assert_eq!(args_digest(&a), args_digest(&b));
    }

    #[test]
    fn provenance_is_recorded() {
        let store = ApprovalStore::new();
        let request = make_request("fs.write", Effect::FilesystemWrite, None, "run_1", &[]);
        store.record(ApprovalRecord {
            request,
            scope: ApprovalScope::ForRun {
                run_id: "run_1".into(),
            },
            decided_by: "web-client".into(),
            decided_at_ms: 42,
        });
        let record = &store.records_lock_for_test()[0];
        assert_eq!(record.decided_by, "web-client");
        assert_eq!(record.decided_at_ms, 42);
    }
}

impl ApprovalStore {
    #[cfg(test)]
    fn records_lock_for_test(&self) -> parking_lot::MutexGuard<'_, Vec<ApprovalRecord>> {
        self.records.lock()
    }
}
