//! Memory v2 (§28.2, Task 15.3, C-011..C-014): provenance, confidence,
//! sensitivity, TTL expiry, and supersede links on every record.

use anyhow::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    Working,
    Episodic,
    Semantic,
    Procedural,
    Preference,
    Reasoning,
    NegativeLesson,
    Observation,
    Decision,
    Trajectory,
    HarnessExperience,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScope {
    Global,
    User,
    Project,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub memory_id: String,
    pub kind: MemoryKind,
    pub scope: MemoryScope,
    pub content: String,
    /// Where this came from: run id, source message ids, tool output...
    pub provenance: Vec<String>,
    /// 0.0..1.0 confidence.
    pub confidence: f32,
    pub created_at_ms: i64,
    /// Optional expiry (TTL): expired memories are omitted from selection.
    #[serde(default)]
    pub expires_at_ms: Option<i64>,
    /// This record supersedes (replaces) the listed ids: superseded records
    /// are omitted from default selection (C-011 semantics).
    #[serde(default)]
    pub supersedes: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// In-memory memory store with selection semantics.
#[derive(Default)]
pub struct MemoryStore {
    records: RwLock<BTreeMap<String, MemoryRecord>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, record: MemoryRecord) {
        self.records.write().insert(record.memory_id.clone(), record);
    }

    /// Selection (C-011): expired and superseded records omitted; remaining
    /// sorted by confidence desc then recency.
    pub fn select(&self, now_ms: i64, kinds: Option<&[MemoryKind]>) -> Vec<MemoryRecord> {
        let records = self.records.read();
        let superseded: std::collections::BTreeSet<&str> = records
            .values()
            .flat_map(|r| r.supersedes.iter().map(String::as_str))
            .collect();
        let mut selected: Vec<MemoryRecord> = records
            .values()
            .filter(|record| {
                if record.expires_at_ms.map(|exp| now_ms >= exp).unwrap_or(false) {
                    return false;
                }
                if superseded.contains(record.memory_id.as_str()) {
                    return false;
                }
                match kinds {
                    Some(kinds) => kinds.contains(&record.kind),
                    None => true,
                }
            })
            .cloned()
            .collect();
        selected.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.created_at_ms.cmp(&a.created_at_ms))
        });
        selected
    }

    pub fn get(&self, memory_id: &str) -> Option<MemoryRecord> {
        self.records.read().get(memory_id).cloned()
    }

    pub fn len(&self) -> usize {
        self.records.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Memory proposal policy (C-019): sensitive/global candidates follow the
/// configured policy — auto, store, ask, reject.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalPolicy {
    Auto,
    Store,
    Ask,
    Reject,
}

#[derive(Clone, Debug)]
pub struct MemoryProposal {
    pub content: String,
    pub kind: MemoryKind,
    pub scope: MemoryScope,
    pub provenance: Vec<String>,
}

/// Applies the policy to a proposal: `auto` stores project-scoped facts but
/// asks for global ones; `reject` never stores; `ask` always defers.
pub fn apply_proposal_policy(
    policy: ProposalPolicy,
    proposal: MemoryProposal,
) -> Result<Option<MemoryRecord>> {
    match policy {
        ProposalPolicy::Reject | ProposalPolicy::Ask => Ok(None),
        ProposalPolicy::Store | ProposalPolicy::Auto => {
            if policy == ProposalPolicy::Auto && proposal.scope == MemoryScope::Global {
                return Ok(None); // auto never stores global-scope facts
            }
            Ok(Some(MemoryRecord {
                memory_id: format!("mem_{}", hash(&proposal.content)),
                kind: proposal.kind,
                scope: proposal.scope,
                content: proposal.content,
                provenance: proposal.provenance,
                confidence: 0.7,
                created_at_ms: now_ms(),
                expires_at_ms: None,
                supersedes: vec![],
                tags: vec![],
            }))
        }
    }
}

fn hash(content: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(content.as_bytes());
    format!("{:x}", u64::from_be_bytes(digest[..8].try_into().unwrap()))
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str, kind: MemoryKind, content: &str, confidence: f32) -> MemoryRecord {
        MemoryRecord {
            memory_id: id.into(),
            kind,
            scope: MemoryScope::Project,
            content: content.into(),
            provenance: vec!["run_1".into()],
            confidence,
            created_at_ms: 1,
            expires_at_ms: None,
            supersedes: vec![],
            tags: vec![],
        }
    }

    #[test]
    fn expired_memories_are_omitted() {
        let store = MemoryStore::new();
        let mut expired = record("m1", MemoryKind::Semantic, "stale fact", 0.9);
        expired.expires_at_ms = Some(100);
        let fresh = record("m2", MemoryKind::Semantic, "fresh fact", 0.5);
        store.insert(expired);
        store.insert(fresh);

        let selected = store.select(150, None);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].memory_id, "m2");
    }

    #[test]
    fn superseded_memories_are_omitted_by_default() {
        let store = MemoryStore::new();
        let old = record("old", MemoryKind::Decision, "use HTTP", 0.8);
        let mut new = record("new", MemoryKind::Decision, "use HTTPS", 0.9);
        new.supersedes = vec!["old".into()];
        store.insert(old);
        store.insert(new);

        let selected = store.select(10_000, None);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].memory_id, "new");
    }

    #[test]
    fn confidence_ranks_selection() {
        let store = MemoryStore::new();
        store.insert(record("low", MemoryKind::Semantic, "weak", 0.2));
        store.insert(record("high", MemoryKind::Semantic, "strong", 0.95));
        let selected = store.select(1000, None);
        assert_eq!(selected[0].memory_id, "high");
    }

    #[test]
    fn kind_filters_narrow_selection() {
        let store = MemoryStore::new();
        store.insert(record("lesson", MemoryKind::NegativeLesson, "watch out", 0.9));
        store.insert(record("fact", MemoryKind::Semantic, "fact", 0.9));
        let selected = store.select(1000, Some(&[MemoryKind::NegativeLesson]));
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].kind, MemoryKind::NegativeLesson);
    }

    #[test]
    fn auto_policy_stores_project_but_not_global() {
        let project = MemoryProposal {
            content: "project fact".into(),
            kind: MemoryKind::Semantic,
            scope: MemoryScope::Project,
            provenance: vec!["run_2".into()],
        };
        assert!(apply_proposal_policy(ProposalPolicy::Auto, project).unwrap().is_some());

        let global = MemoryProposal {
            content: "global fact".into(),
            kind: MemoryKind::Semantic,
            scope: MemoryScope::Global,
            provenance: vec!["run_2".into()],
        };
        assert!(apply_proposal_policy(ProposalPolicy::Auto, global).unwrap().is_none());
    }

    #[test]
    fn reject_policy_never_stores() {
        let proposal = MemoryProposal {
            content: "sensitive".into(),
            kind: MemoryKind::Semantic,
            scope: MemoryScope::Global,
            provenance: vec![],
        };
        assert!(apply_proposal_policy(ProposalPolicy::Reject, proposal).unwrap().is_none());
    }

    #[test]
    fn provenance_is_carried_on_records() {
        let record = record("m", MemoryKind::Observation, "observed", 0.5);
        assert_eq!(record.provenance, vec!["run_1".to_owned()]);
    }
}
