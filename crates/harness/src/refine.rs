//! Dream / Refine pipeline (§30, Tasks 17.1-17.6, C-020, H-001..H-010).
//!
//! Ported from PrimeIntellect-ai/prime-agent@844e85545af6858dcb3d6cfe42bbfcf2ca0be4e5
//! refinement.ts entry model (MIT). Modified for Steward: HarnessEntry
//! records (prompt/memory/skill/subagent refinements) persist in the
//! Git-backed ContextRepo under `entries/`, with apply/rollback via repo
//! commits.
//!
//! Nightly dream normalizes trajectories into candidate improvements
//! (prompt notes, skills, rules). Candidates NEVER activate automatically:
//! they pass a policy gate (`activation: manual` default) and a regression
//! eval check before a Git-backed commit flips the snapshot pointer.
use crate::context_repo::ContextRepo;
use anyhow::{Context as _, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

/// What a candidate proposes to change.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CandidateKind {
    PromptNote { file: String },
    Rule { file: String },
    Skill { skill_id: String },
    AgentProfileTweak { profile_id: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub candidate_id: String,
    pub kind: CandidateKind,
    pub title: String,
    pub rationale: String,
    /// New file content (Git-committable).
    pub content: String,
    /// Source trajectory/observation ids (H-006 provenance).
    pub evidence: Vec<String>,
    pub status: CandidateStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateStatus {
    Proposed,
    Evaluated,
    Activated,
    Rejected,
}

/// Refinement policy (Task 17.4): defaults are manual activation with
/// zero-tolerance for score regressions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RefinementPolicy {
    pub score_regression_max: f32,
    pub cost_regression_percent_max: f32,
    pub activation: Activation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Activation {
    Manual,
    AutoIfGatesPass,
}

impl Default for RefinementPolicy {
    fn default() -> Self {
        Self {
            score_regression_max: 0.0,
            cost_regression_percent_max: 10.0,
            activation: Activation::Manual,
        }
    }
}

/// Eval result comparing baseline vs candidate (H-007/H-008).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvalComparison {
    pub baseline_score: f32,
    pub candidate_score: f32,
    pub baseline_cost_microusd: u64,
    pub candidate_cost_microusd: u64,
}

impl EvalComparison {
    pub fn score_delta(&self) -> f32 {
        self.candidate_score - self.baseline_score
    }

    pub fn cost_regression_percent(&self) -> f32 {
        if self.baseline_cost_microusd == 0 {
            return 0.0;
        }
        ((self.candidate_cost_microusd as f64 - self.baseline_cost_microusd as f64)
            / self.baseline_cost_microusd as f64
            * 100.0) as f32
    }

    fn gates_pass(&self, policy: &RefinementPolicy) -> bool {
        self.score_delta() >= -policy.score_regression_max
            && self.cost_regression_percent() <= policy.cost_regression_percent_max
    }
}

struct Inner {
    candidates: BTreeMap<String, Candidate>,
    evals: BTreeMap<String, EvalComparison>,
    counter: u64,
}

/// The refinement pipeline.
pub struct RefinePipeline {
    repo: ContextRepo,
    policy: RefinementPolicy,
    inner: Mutex<Inner>,
}

impl RefinePipeline {
    pub fn new(repo: ContextRepo, policy: RefinementPolicy) -> Self {
        Self {
            repo,
            policy,
            inner: Mutex::new(Inner {
                candidates: BTreeMap::new(),
                evals: BTreeMap::new(),
                counter: 0,
            }),
        }
    }

    /// Creates a candidate from trajectory evidence (H-001/H-004). The
    /// candidate can only write inside the harness repo — never production.
    pub fn propose(
        &self,
        kind: CandidateKind,
        title: &str,
        rationale: &str,
        content: &str,
        evidence: &[String],
    ) -> Result<Candidate> {
        let mut inner = self.inner.lock();
        inner.counter += 1;
        let candidate = Candidate {
            candidate_id: format!("cand_{:04}", inner.counter),
            kind,
            title: title.to_owned(),
            rationale: rationale.to_owned(),
            content: content.to_owned(),
            evidence: evidence.to_vec(),
            status: CandidateStatus::Proposed,
        };
        inner
            .candidates
            .insert(candidate.candidate_id.clone(), candidate.clone());
        Ok(candidate)
    }

    /// Attaches eval results (gate input).
    pub fn record_eval(&self, candidate_id: &str, comparison: EvalComparison) -> Result<()> {
        let mut inner = self.inner.lock();
        let candidate = inner
            .candidates
            .get_mut(candidate_id)
            .context("candidate not found")?;
        candidate.status = CandidateStatus::Evaluated;
        inner.evals.insert(candidate_id.to_owned(), comparison);
        Ok(())
    }

    /// Activates a candidate: policy gate + eval gates must pass. With
    /// `Activation::Manual`, auto activation is refused (§30.3: nightly jobs
    /// can never silently alter harness behavior). Activation writes the
    /// content into the Git-backed repo and returns the commit.
    pub fn activate(&self, candidate_id: &str, requested_by: Activation) -> Result<String> {
        let mut inner = self.inner.lock();
        {
            let candidate = inner
                .candidates
                .get(candidate_id)
                .context("candidate not found")?;
            if candidate.status == CandidateStatus::Activated {
                anyhow::bail!("candidate already activated");
            }
        }
        // Manual policy: the request must come from a human surface.
        if self.policy.activation == Activation::Manual
            && requested_by == Activation::AutoIfGatesPass
        {
            anyhow::bail!("activation policy is manual; nightly/auto requests are refused");
        }
        // Eval gate: candidates must have evals on record and pass them.
        let comparison = inner
            .evals
            .get(candidate_id)
            .cloned()
            .context("no eval recorded; run the eval suite before activation")?;
        let (candidate, target_path) = {
            let candidate = inner
                .candidates
                .get(candidate_id)
                .context("candidate not found")?;
            let target_path = match &candidate.kind {
                CandidateKind::PromptNote { file } => format!("prompts/{file}"),
                CandidateKind::Rule { file } => format!("rules/{file}"),
                CandidateKind::Skill { skill_id } => format!("skills/{skill_id}/SKILL.md"),
                CandidateKind::AgentProfileTweak { profile_id } => {
                    format!("agents/{profile_id}.yaml")
                }
            };
            (candidate.clone(), target_path)
        };
        if !comparison.gates_pass(&self.policy) {
            inner.candidates.get_mut(candidate_id).unwrap().status = CandidateStatus::Rejected;
            anyhow::bail!(
                "eval gates failed: score_delta {:.3}, cost_regression {:.1}%",
                comparison.score_delta(),
                comparison.cost_regression_percent()
            );
        }
        // Commit into the Git-backed harness repo (H-010 atomic activation).
        let commit = self.repo.write(
            &target_path,
            &candidate.content,
            &format!("refine: {}", candidate.title),
        )?;
        inner.candidates.get_mut(candidate_id).unwrap().status = CandidateStatus::Activated;
        Ok(commit)
    }

    pub fn candidate(&self, candidate_id: &str) -> Option<Candidate> {
        self.inner.lock().candidates.get(candidate_id).cloned()
    }

    pub fn candidates(&self) -> Vec<Candidate> {
        self.inner.lock().candidates.values().cloned().collect()
    }

    /// Human-readable proposal for review (H-009).
    pub fn proposal_for_review(&self, candidate_id: &str) -> Result<String> {
        let inner = self.inner.lock();
        let candidate = inner
            .candidates
            .get(candidate_id)
            .context("candidate not found")?;
        let target_path = match &candidate.kind {
            CandidateKind::PromptNote { file } => format!("prompts/{file}"),
            CandidateKind::Rule { file } => format!("rules/{file}"),
            CandidateKind::Skill { skill_id } => format!("skills/{skill_id}/SKILL.md"),
            CandidateKind::AgentProfileTweak { profile_id } => format!("agents/{profile_id}.yaml"),
        };
        Ok(format!(
            "--- proposed {} ---\n{}",
            target_path, candidate.content
        ))
    }
}

/// Nightly dream (C-020): processes trajectories into candidate proposals
/// with dedup by failure signature. Default output: proposals only;
/// activation stays manual.
pub fn dream(pipeline: &RefinePipeline, trajectories: &[Trajectory]) -> Result<Vec<Candidate>> {
    let mut seen: BTreeMap<String, ()> = BTreeMap::new();
    let mut candidates = Vec::new();
    for trajectory in trajectories {
        if trajectory.outcome != TrajectoryOutcome::RepeatedFailure {
            continue;
        }
        if seen
            .insert(trajectory.error_signature.clone(), ())
            .is_some()
        {
            continue; // dedup identical failure patterns (H-006)
        }
        candidates.push(pipeline.propose(
            CandidateKind::Rule {
                file: format!("{}.rule.md", slug(&trajectory.error_signature)),
            },
            &format!("Handle {}", trajectory.error_signature),
            &format!(
                "Repeated failure pattern observed {} times",
                trajectory.occurrences
            ),
            &trajectory.correction,
            &trajectory.evidence,
        )?);
    }
    Ok(candidates)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trajectory {
    pub error_signature: String,
    pub correction: String,
    pub occurrences: u32,
    pub outcome: TrajectoryOutcome,
    pub evidence: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrajectoryOutcome {
    RepeatedFailure,
    Success,
}

fn slug(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pipeline() -> (tempfile::TempDir, RefinePipeline) {
        let temp = tempfile::tempdir().unwrap();
        let repo = ContextRepo::init(temp.path()).unwrap();
        (temp, RefinePipeline::new(repo, RefinementPolicy::default()))
    }

    fn good_eval() -> EvalComparison {
        EvalComparison {
            baseline_score: 0.70,
            candidate_score: 0.85,
            baseline_cost_microusd: 1_000,
            candidate_cost_microusd: 1_000,
        }
    }

    #[test]
    fn propose_creates_provenanced_candidates() {
        let (_temp, pipeline) = pipeline();
        let candidate = pipeline
            .propose(
                CandidateKind::PromptNote {
                    file: "rust-notes.md".into(),
                },
                "Handle linker errors",
                "seen 3 times",
                "Run cargo build -v first.",
                &["traj_1".into(), "traj_2".into()],
            )
            .unwrap();
        assert_eq!(candidate.status, CandidateStatus::Proposed);
        assert_eq!(candidate.evidence.len(), 2, "provenance recorded");
    }

    #[test]
    fn activation_requires_evals_and_manual_gate() {
        let (_temp, pipeline) = pipeline();
        let candidate = pipeline
            .propose(
                CandidateKind::Rule {
                    file: "rust.rule.md".into(),
                },
                "fmt before done",
                "repeated clippy failures",
                "Run cargo fmt and clippy before completion.",
                &["traj_9".into()],
            )
            .unwrap();

        // No eval on record: refused.
        assert!(pipeline
            .activate(&candidate.candidate_id, Activation::Manual)
            .is_err());

        // Eval recorded and passing: manual activation works, writes to repo.
        pipeline
            .record_eval(&candidate.candidate_id, good_eval())
            .unwrap();
        let commit = pipeline
            .activate(&candidate.candidate_id, Activation::Manual)
            .unwrap();
        assert!(!commit.is_empty());
        assert_eq!(
            pipeline.candidate(&candidate.candidate_id).unwrap().status,
            CandidateStatus::Activated
        );
    }

    #[test]
    fn manual_policy_refuses_auto_activation() {
        let (_temp, pipeline) = pipeline();
        let candidate = pipeline
            .propose(
                CandidateKind::Rule {
                    file: "safe.rule.md".into(),
                },
                "t",
                "r",
                "content",
                &[],
            )
            .unwrap();
        pipeline
            .record_eval(&candidate.candidate_id, good_eval())
            .unwrap();
        let error = pipeline
            .activate(&candidate.candidate_id, Activation::AutoIfGatesPass)
            .unwrap_err();
        assert!(error.to_string().contains("manual"), "got {error}");
        // §30.3: nightly jobs can never silently alter harness behavior.
    }

    #[test]
    fn failing_evals_reject_candidate() {
        let (_temp, pipeline) = pipeline();
        let candidate = pipeline
            .propose(
                CandidateKind::Rule {
                    file: "bad.rule.md".into(),
                },
                "t",
                "r",
                "content",
                &[],
            )
            .unwrap();
        pipeline
            .record_eval(
                &candidate.candidate_id,
                EvalComparison {
                    baseline_score: 0.9,
                    candidate_score: 0.5,
                    baseline_cost_microusd: 100,
                    candidate_cost_microusd: 500,
                },
            )
            .unwrap();
        assert!(pipeline
            .activate(&candidate.candidate_id, Activation::Manual)
            .is_err());
        assert_eq!(
            pipeline.candidate(&candidate.candidate_id).unwrap().status,
            CandidateStatus::Rejected
        );
    }

    #[test]
    fn dream_dedupes_repeated_failure_patterns() {
        let (_temp, pipeline) = pipeline();
        let trajectories = vec![
            Trajectory {
                error_signature: "rust linker".into(),
                correction: "Run cargo build -v and check .dll ordering".into(),
                occurrences: 3,
                outcome: TrajectoryOutcome::RepeatedFailure,
                evidence: vec!["t1".into()],
            },
            Trajectory {
                error_signature: "rust linker".into(),
                correction: "Same fix".into(),
                occurrences: 2,
                outcome: TrajectoryOutcome::RepeatedFailure,
                evidence: vec!["t2".into()],
            },
            Trajectory {
                error_signature: "none".into(),
                correction: "n/a".into(),
                occurrences: 0,
                outcome: TrajectoryOutcome::Success,
                evidence: vec![],
            },
        ];
        let candidates = dream(&pipeline, &trajectories).unwrap();
        assert_eq!(candidates.len(), 1, "deduped: one pattern = one candidate");
        assert_eq!(candidates[0].evidence, vec!["t1".to_owned()]);
    }

    #[test]
    fn activated_content_lands_in_git_repo() {
        let (temp, pipeline) = pipeline();
        let candidate = pipeline
            .propose(
                CandidateKind::Rule {
                    file: "git.rule.md".into(),
                },
                "rebase",
                "r",
                "Always rebase before merge.",
                &[],
            )
            .unwrap();
        pipeline
            .record_eval(&candidate.candidate_id, good_eval())
            .unwrap();
        pipeline
            .activate(&candidate.candidate_id, Activation::Manual)
            .unwrap();
        let on_disk = std::fs::read_to_string(temp.path().join("rules/git.rule.md")).unwrap();
        assert!(on_disk.contains("rebase before merge"));
    }

    #[test]
    fn eval_comparison_calculates_deltas() {
        let comparison = good_eval();
        assert!((comparison.score_delta() - 0.15).abs() < 1e-6);
        assert!((comparison.cost_regression_percent() - 0.0).abs() < 1e-6);
    }
}

// ── HarnessEntry store (prime refinement.ts port) ─────────────────────────

/// What kind of refinement an entry captures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefinementKind {
    Prompt,
    Memory,
    Skill,
    Subagent,
}

/// One durable refinement entry: a titled change with payload, snapshotted
/// in the Git-backed context repo so it can be applied and rolled back.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HarnessEntry {
    pub id: String,
    pub kind: RefinementKind,
    pub title: String,
    pub created_at_unix: u64,
    /// ContextRepo commit the entry was written at (snapshot reference).
    pub snapshot_ref: String,
    /// Entry payload (diff text, prompt text, skill id, subagent spec…).
    pub payload: String,
}

/// Lists entry ids recorded in the repo's `entries/` directory.
pub fn list_entries(repo: &ContextRepo) -> Result<Vec<HarnessEntry>> {
    // Entry metadata is one JSON document per entry; the log lists commits.
    let index = repo
        .read("entries/index.json")
        .unwrap_or_else(|_| "[]".to_owned());
    Ok(serde_json::from_str(&index).context("entries index is corrupt")?)
}

/// Appends an entry: writes payload + metadata and commits (the commit is
/// the snapshot reference).
pub fn append_entry(
    repo: &ContextRepo,
    kind: RefinementKind,
    title: &str,
    payload: &str,
) -> Result<HarnessEntry> {
    let mut entries = list_entries(repo)?;
    let id = format!(
        "entry-{}-{}",
        entries.len() + 1,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    );
    let created_at_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let snapshot_ref = repo.write(
        &format!("entries/{id}.txt"),
        payload,
        &format!("harness entry: {title}"),
    )?;
    let entry = HarnessEntry {
        id,
        kind,
        title: title.to_owned(),
        created_at_unix,
        snapshot_ref,
        payload: payload.to_owned(),
    };
    entries.push(entry.clone());
    repo.write(
        "entries/index.json",
        &serde_json::to_string_pretty(&entries)?,
        &format!("harness entry index: {}", entry.id),
    )?;
    Ok(entry)
}

/// Applies an entry: re-writes its payload at HEAD (idempotent apply) and
/// returns the diff against the pre-apply snapshot.
pub fn apply_entry(repo: &ContextRepo, entry_id: &str) -> Result<String> {
    let entries = list_entries(repo)?;
    let entry = entries
        .iter()
        .find(|entry| entry.id == entry_id)
        .context("entry not found")?;
    let path = format!("entries/{}.txt", entry.id);
    let before = repo.head()?;
    repo.write(&path, &entry.payload, &format!("apply entry {}", entry.id))?;
    repo.diff(&before, &repo.head()?, &path)
}

/// Rolls an entry back to its original snapshot commit.
pub fn rollback_entry(repo: &ContextRepo, entry_id: &str) -> Result<()> {
    let entries = list_entries(repo)?;
    let entry = entries
        .iter()
        .find(|entry| entry.id == entry_id)
        .context("entry not found")?;
    let path = format!("entries/{}.txt", entry.id);
    repo.rollback(
        &path,
        &entry.snapshot_ref,
        &format!("rollback entry {}", entry.id),
    )?;
    Ok(())
}

#[cfg(test)]
mod harness_entry_tests {
    use super::*;
    use crate::context_repo::ContextRepo;
    use std::path::PathBuf;

    fn test_repo() -> (tempfile::TempDir, ContextRepo) {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let repo = ContextRepo::init(&dir.path().to_path_buf()).expect("init repo");
        (dir, repo)
    }

    #[test]
    fn entries_round_trip_through_the_repo() {
        let (_guard, repo) = test_repo();
        let entry = append_entry(
            &repo,
            RefinementKind::Prompt,
            "tighten search prompt",
            "be terse",
        );
        assert!(entry.is_ok(), "entry append: {entry:?}");
        let entries = list_entries(&repo).expect("list");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "tighten search prompt");
        assert_eq!(entries[0].kind, RefinementKind::Prompt);
    }

    #[test]
    fn apply_then_rollback_restores_snapshot() {
        let (_guard, repo) = test_repo();
        let entry = append_entry(
            &repo,
            RefinementKind::Memory,
            "memory prune rule",
            "keep 30d",
        )
        .expect("append");
        // Apply returns a diff (content identical to snapshot → possibly empty).
        let _ = apply_entry(&repo, &entry.id).expect("apply");
        // Corrupt the payload file, then roll back to the entry snapshot.
        repo.write(&format!("entries/{}.txt", entry.id), "corrupted", "damage")
            .expect("write damage");
        rollback_entry(&repo, &entry.id).expect("rollback");
        let restored = repo
            .read(&format!("entries/{}.txt", entry.id))
            .expect("read restored");
        assert_eq!(restored, "keep 30d");
    }
}
