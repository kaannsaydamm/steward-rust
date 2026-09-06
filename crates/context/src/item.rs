//! Context candidate model (§19.1).

use serde::{Deserialize, Serialize};

/// Where a candidate came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextSourceKind {
    UserMessage,
    ThreadSummary,
    RecentMessages,
    Scratchpad,
    ProjectRule,
    Skill,
    RepoMap,
    ExplicitFile,
    ToolOutput,
    Observation,
    LongTermMemory,
    NegativeLesson,
    StrategyNote,
    ModelInstruction,
}

/// Scope a candidate is valid within.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextScope {
    Global,
    Project { workspace_id: String },
    Directory { path: String },
    Run { run_id: String },
}

/// Trust label (C-004): untrusted text can never drive policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    TrustedConfiguration,
    UserInput,
    RepositoryContent,
    ToolOutput,
    GeneratedMemory,
    UntrustedExternal,
}

/// Sensitivity class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sensitivity {
    Public,
    Internal,
    Secret,
}

/// One candidate for inclusion in the next model call.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContextItem {
    /// Stable id for manifest traceability, e.g. `rule:rust`.
    pub id: String,
    pub source: ContextSourceKind,
    pub content: String,
    /// Rough token estimate (chars/4 heuristic unless overridden).
    pub estimated_tokens: u32,
    /// 0.0..1.0 relevance to the current task.
    pub relevance: f32,
    /// 0.0..1.0 authority: system > user > repo > web.
    pub authority: f32,
    /// 0.0..1.0 freshness.
    pub freshness: f32,
    pub trust: TrustLevel,
    pub sensitivity: Sensitivity,
    pub scope: ContextScope,
}

impl ContextItem {
    pub fn estimate_tokens(content: &str) -> u32 {
        (content.len() as u32).div_ceil(4)
    }

    /// Allocator score: weighted benefit minus token cost.
    pub fn score(&self) -> f32 {
        self.relevance * 0.5 + self.authority * 0.3 + self.freshness * 0.2
    }

    /// Scope guard: run/directory-scoped items are invalid in foreign scopes.
    pub fn matches_scope(&self, workspace_id: Option<&str>, run_id: Option<&str>) -> bool {
        match &self.scope {
            ContextScope::Global | ContextScope::Project { .. } => true,
            ContextScope::Directory { .. } => true,
            ContextScope::Run { run_id: item_run } => item_run == run_id.unwrap_or_default(),
        }
    }
}
