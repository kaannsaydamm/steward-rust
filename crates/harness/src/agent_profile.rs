//! AgentProfile: declarative agent behavior as data (§26.1, Task 11.1,
//! K-010). No Rust code for a new profile — YAML/Markdown only.

use serde::{Deserialize, Serialize};
use steward_kernel::budget::Budget;
use steward_tools::effects::Effect;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelPolicy {
    Auto,
    /// Capability class hint for the router (`fast-reasoning`, ...).
    FastReasoning,
    Best,
    Local,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextMode {
    /// Fresh context; inherits only listed items.
    Fresh,
    /// Full parent context copy.
    Inherit,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpawnMode {
    #[default]
    /// Parent awaits the child directly.
    Inline,
    /// Parent continues; child returns a task id.
    Async,
    /// Child outlives the parent turn under run policy.
    Detached,
    /// Started by scheduler/cron.
    Scheduled,
    /// Joins a named collaboration group.
    Team,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentProfile {
    pub schema_version: u32,
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub model: ModelPolicy,
    pub context: ProfileContext,
    pub tools: ProfileTools,
    pub workspace: ProfileWorkspace,
    #[serde(default)]
    pub budget: Budget,
    #[serde(default)]
    pub spawn: SpawnMode,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProfileContext {
    pub mode: ContextMode,
    /// What a fresh context inherits: task, workspace_rules...
    #[serde(default)]
    pub inherit: Vec<String>,
    #[serde(default)]
    pub memory: ProfileMemory,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProfileMemory {
    pub project: MemoryAccess,
    pub user: MemoryAccess,
}

impl Default for MemoryAccess {
    fn default() -> Self {
        MemoryAccess::None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryAccess {
    Read,
    ReadWrite,
    None,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProfileTools {
    /// Tool discovery (search) enabled.
    pub discovery: bool,
    /// Effects this profile may be granted. Validation rejects unknown
    /// grants; the runtime policy still gates per call.
    pub allow_effects: Vec<Effect>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProfileWorkspace {
    pub mode: steward_workspace::lease::WorkspaceMode,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ProfileError {
    #[error("profile id '{0}' is invalid: use lowercase ASCII, digits, '-'")]
    InvalidId(String),
    #[error("effect grant {0} cannot be granted to a read-only workspace profile")]
    WriteEffectOnReadOnly(&'static str),
    #[error("schema version {0} is not supported (expected 1)")]
    UnsupportedVersion(u32),
}

impl AgentProfile {
    pub fn parse_yaml(source: &str) -> Result<Self, serde_yaml::Error> {
        let profile: Self = serde_yaml::from_str(source)?;
        Ok(profile)
    }

    pub fn validate(&self) -> Result<(), ProfileError> {
        if self.schema_version != 1 {
            return Err(ProfileError::UnsupportedVersion(self.schema_version));
        }
        let valid = !self.id.is_empty()
            && self
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
        if !valid {
            return Err(ProfileError::InvalidId(self.id.clone()));
        }
        // D-017: a read-only workspace can never receive write effects.
        let read_only = matches!(
            self.workspace.mode,
            steward_workspace::lease::WorkspaceMode::ReadOnly
        );
        if read_only {
            for effect in &self.tools.allow_effects {
                if matches!(
                    effect,
                    Effect::FilesystemWrite
                        | Effect::FilesystemDelete
                        | Effect::GitWrite
                        | Effect::ProcessExecute
                        | Effect::MemoryWrite
                        | Effect::HarnessModify
                ) {
                    return Err(ProfileError::WriteEffectOnReadOnly(effect.as_str()));
                }
            }
        }
        Ok(())
    }
}

/// The built-in profile set (§26.2): explorer, worker, reviewer, verifier,
/// researcher. Shipped as data; users can add more without code.
pub fn builtin_profiles() -> Vec<AgentProfile> {
    vec![
        explorer(),
        worker(),
        reviewer(),
        verifier(),
        researcher(),
    ]
}

/// D-017: read-only reconnaissance; write/process/network effects denied.
pub fn explorer() -> AgentProfile {
    AgentProfile {
        schema_version: 1,
        id: "explorer".into(),
        title: "Repository Explorer".into(),
        description: "Read-only repository reconnaissance and evidence gathering.".into(),
        model: ModelPolicy::FastReasoning,
        context: ProfileContext {
            mode: ContextMode::Fresh,
            inherit: vec!["task".into(), "workspace_rules".into()],
            memory: ProfileMemory { project: MemoryAccess::Read, user: MemoryAccess::None },
        },
        tools: ProfileTools {
            discovery: true,
            allow_effects: vec![Effect::FilesystemRead, Effect::GitRead],
        },
        workspace: ProfileWorkspace { mode: steward_workspace::lease::WorkspaceMode::ReadOnly },
        budget: Budget {
            max_model_calls: Some(8),
            max_tool_calls: Some(30),
            ..Default::default()
        },
        spawn: SpawnMode::Inline,
    }
}

pub fn worker() -> AgentProfile {
    AgentProfile {
        schema_version: 1,
        id: "worker".into(),
        title: "Coding Worker".into(),
        description: "Implements changes inside its worktree lease.".into(),
        model: ModelPolicy::Auto,
        context: ProfileContext {
            mode: ContextMode::Fresh,
            inherit: vec!["task".into(), "workspace_rules".into(), "diff".into()],
            memory: ProfileMemory { project: MemoryAccess::Read, user: MemoryAccess::None },
        },
        tools: ProfileTools {
            discovery: true,
            allow_effects: vec![
                Effect::FilesystemRead,
                Effect::FilesystemWrite,
                Effect::GitRead,
                Effect::GitWrite,
                Effect::ProcessExecute,
            ],
        },
        workspace: ProfileWorkspace { mode: steward_workspace::lease::WorkspaceMode::Worktree },
        budget: Budget {
            max_model_calls: Some(64),
            max_tool_calls: Some(256),
            ..Default::default()
        },
        spawn: SpawnMode::Inline,
    }
}

pub fn reviewer() -> AgentProfile {
    AgentProfile {
        schema_version: 1,
        id: "reviewer".into(),
        title: "Code Reviewer".into(),
        description: "Reviews diffs for correctness and policy issues.".into(),
        model: ModelPolicy::Best,
        context: ProfileContext {
            mode: ContextMode::Inherit,
            inherit: vec![],
            memory: ProfileMemory { project: MemoryAccess::Read, user: MemoryAccess::None },
        },
        tools: ProfileTools {
            discovery: true,
            allow_effects: vec![Effect::FilesystemRead, Effect::GitRead],
        },
        workspace: ProfileWorkspace { mode: steward_workspace::lease::WorkspaceMode::ReadOnly },
        budget: Budget { max_model_calls: Some(16), ..Default::default() },
        spawn: SpawnMode::Inline,
    }
}

pub fn verifier() -> AgentProfile {
    AgentProfile {
        schema_version: 1,
        id: "verifier".into(),
        title: "Verifier".into(),
        description: "Runs tests/commands and reports evidence.".into(),
        model: ModelPolicy::FastReasoning,
        context: ProfileContext {
            mode: ContextMode::Fresh,
            inherit: vec!["task".into()],
            memory: ProfileMemory { project: MemoryAccess::Read, user: MemoryAccess::None },
        },
        tools: ProfileTools {
            discovery: true,
            allow_effects: vec![
                Effect::FilesystemRead,
                Effect::ProcessExecute,
                Effect::GitRead,
            ],
        },
        workspace: ProfileWorkspace { mode: steward_workspace::lease::WorkspaceMode::ReadWrite },
        budget: Budget {
            max_model_calls: Some(12),
            max_tool_calls: Some(48),
            ..Default::default()
        },
        spawn: SpawnMode::Inline,
    }
}

pub fn researcher() -> AgentProfile {
    AgentProfile {
        schema_version: 1,
        id: "researcher".into(),
        title: "Researcher".into(),
        description: "Web and documentation research with citations.".into(),
        model: ModelPolicy::Auto,
        context: ProfileContext {
            mode: ContextMode::Fresh,
            inherit: vec!["task".into()],
            memory: ProfileMemory { project: MemoryAccess::Read, user: MemoryAccess::Read },
        },
        tools: ProfileTools {
            discovery: true,
            allow_effects: vec![Effect::NetworkHttp, Effect::NetworkBrowser],
        },
        workspace: ProfileWorkspace { mode: steward_workspace::lease::WorkspaceMode::ReadOnly },
        budget: Budget {
            max_model_calls: Some(20),
            max_tool_calls: Some(64),
            ..Default::default()
        },
        spawn: SpawnMode::Async,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPLORER_YAML: &str = r#"
schema_version: 1
id: explorer
title: Repository Explorer
description: Read-only reconnaissance
model: fast_reasoning
context:
  mode: fresh
  inherit: [task, workspace_rules]
  memory:
    project: read
    user: none
tools:
  discovery: true
  allow_effects: [filesystem_read, git_read]
workspace:
  mode: read_only
budget:
  max_model_calls: 8
  max_tool_calls: 30
"#;

    #[test]
    fn parses_yaml_profile_without_code() {
        let profile = AgentProfile::parse_yaml(EXPLORER_YAML).unwrap();
        assert_eq!(profile.id, "explorer");
        assert_eq!(profile.model, ModelPolicy::FastReasoning);
        assert_eq!(profile.tools.allow_effects, vec![Effect::FilesystemRead, Effect::GitRead]);
        assert_eq!(profile.budget.max_model_calls, Some(8));
    }

    #[test]
    fn builtins_validate() {
        for profile in builtin_profiles() {
            profile.validate().unwrap_or_else(|error| panic!("{} invalid: {error}", profile.id));
        }
    }

    #[test]
    fn explorer_cannot_write_even_if_model_requests() {
        // D-017: explorer's effect set excludes writes at the profile level;
        // validation refuses any read-only profile that claims them.
        let mut hostile = explorer();
        hostile.tools.allow_effects.push(Effect::FilesystemWrite);
        let error = hostile.validate().unwrap_err();
        assert!(matches!(error, ProfileError::WriteEffectOnReadOnly("filesystem.write")));
    }

    #[test]
    fn invalid_ids_are_rejected() {
        let mut profile = explorer();
        profile.id = "Bad Id!".into();
        assert!(matches!(profile.validate(), Err(ProfileError::InvalidId(_))));
    }

    #[test]
    fn unsupported_versions_are_rejected() {
        let mut profile = explorer();
        profile.schema_version = 2;
        assert!(matches!(profile.validate(), Err(ProfileError::UnsupportedVersion(2))));
    }

    #[test]
    fn worker_defaults_to_worktree() {
        assert_eq!(worker().workspace.mode, steward_workspace::lease::WorkspaceMode::Worktree);
    }
}
