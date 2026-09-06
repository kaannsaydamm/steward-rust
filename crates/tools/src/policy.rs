//! Effect-based policy (§23.2, Task 7.2, S-001): tool names alias; effects
//! do not. Aliasing a tool can never bypass an effect restriction.

use crate::effects::Effect;
use crate::spec::ToolSpec;
use serde::{Deserialize, Serialize};

/// A scope restricts where an effect may land (glob-ish prefix for fs/git).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EffectScope {
    pub effect: Effect,
    /// Empty = unrestricted. Non-empty = path must fall under one entry.
    #[serde(default)]
    pub allow_paths: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EffectPolicy {
    /// Effects allowed outright.
    pub allowed: Vec<EffectScope>,
    /// Effects denied regardless of any allow entry (deny wins, S-001).
    pub denied: Vec<Effect>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PolicyDecision {
    Allow,
    /// Requires an approval for this exact effect (+ optional path).
    ApprovalRequired {
        effect: Effect,
    },
    Deny {
        effect: Effect,
        reason: String,
    },
}

impl EffectPolicy {
    /// Evaluates every declared effect of the tool. A single denial rejects
    /// the whole call — aliasing the tool name changes nothing.
    pub fn evaluate(&self, tool: &ToolSpec, requested_path: Option<&str>) -> PolicyDecision {
        // Deny list first: monotonic deny (§13.13).
        for effect in &self.denied {
            if tool.declares(effect) {
                return PolicyDecision::Deny {
                    effect: *effect,
                    reason: format!("effect {} is denied by policy", effect.as_str()),
                };
            }
        }
        for effect in &tool.effects {
            match self.effect_allowed(effect, requested_path) {
                true => continue,
                false => return PolicyDecision::ApprovalRequired { effect: *effect },
            }
        }
        PolicyDecision::Allow
    }

    fn effect_allowed(&self, effect: &Effect, requested_path: Option<&str>) -> bool {
        self.allowed.iter().any(|scope| {
            if scope.effect != *effect {
                return false;
            }
            if scope.allow_paths.is_empty() {
                return true;
            }
            match requested_path {
                Some(path) => scope.allow_paths.iter().any(|root| path_under(path, root)),
                None => false,
            }
        })
    }
}

/// Path containment with backslash normalization; refuses prefix tricks
/// (`src_evil` is not under `src`).
fn path_under(path: &str, root: &str) -> bool {
    let path = path.replace('\\', "/");
    let root = root.trim_end_matches('/');
    let root = root.replace('\\', "/").trim_end_matches("/**").to_owned();
    if root.is_empty() {
        return true;
    }
    path == root || path.starts_with(&format!("{root}/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::RiskLevel;
    use serde_json::json;

    fn tool(name: &str, effects: Vec<Effect>) -> ToolSpec {
        ToolSpec {
            id: name.into(),
            title: name.into(),
            description: String::new(),
            schema: json!({}),
            effects,
            risk: RiskLevel::Low,
            tags: vec![],
            provider: "builtin".into(),
        }
    }

    #[test]
    fn aliasing_tool_names_cannot_bypass_effects() {
        // Both tools declare filesystem.write; the policy denies that effect.
        let policy = EffectPolicy {
            allowed: vec![],
            denied: vec![Effect::FilesystemWrite],
        };
        let original = tool(
            "fs.write",
            vec![Effect::FilesystemRead, Effect::FilesystemWrite],
        );
        let alias = tool(
            "write_file_please",
            vec![Effect::FilesystemRead, Effect::FilesystemWrite],
        );

        for candidate in [&original, &alias] {
            let decision = policy.evaluate(candidate, Some("src/main.rs"));
            assert!(
                matches!(
                    decision,
                    PolicyDecision::Deny {
                        effect: Effect::FilesystemWrite,
                        ..
                    }
                ),
                "alias {} must still be denied",
                candidate.id
            );
        }
    }

    #[test]
    fn scoped_allow_requires_path_match() {
        let policy = EffectPolicy {
            allowed: vec![EffectScope {
                effect: Effect::FilesystemWrite,
                allow_paths: vec!["src/**".into()],
            }],
            denied: vec![],
        };
        let writer = tool("fs.write", vec![Effect::FilesystemWrite]);

        assert!(matches!(
            policy.evaluate(&writer, Some("src/lib.rs")),
            PolicyDecision::Allow
        ));
        // `.git` is outside src/** → approval, never silent write (Task 7.3).
        assert!(matches!(
            policy.evaluate(&writer, Some(".git/config")),
            PolicyDecision::ApprovalRequired {
                effect: Effect::FilesystemWrite
            }
        ));
    }

    #[test]
    fn prefix_tricks_do_not_escape_scope() {
        let policy = EffectPolicy {
            allowed: vec![EffectScope {
                effect: Effect::FilesystemWrite,
                allow_paths: vec!["src".into()],
            }],
            denied: vec![],
        };
        let writer = tool("fs.write", vec![Effect::FilesystemWrite]);
        // src_evil is a sibling, not a child of src.
        assert!(matches!(
            policy.evaluate(&writer, Some("src_evil/file.txt")),
            PolicyDecision::ApprovalRequired { .. }
        ));
    }

    #[test]
    fn undeclared_effects_default_to_approval() {
        let policy = EffectPolicy::default();
        let reader = tool("fs.read", vec![Effect::FilesystemRead]);
        assert!(matches!(
            policy.evaluate(&reader, None),
            PolicyDecision::ApprovalRequired {
                effect: Effect::FilesystemRead
            }
        ));
    }

    #[test]
    fn deny_wins_over_allow() {
        let policy = EffectPolicy {
            allowed: vec![EffectScope {
                effect: Effect::GitWrite,
                allow_paths: vec![],
            }],
            denied: vec![Effect::GitWrite],
        };
        let brancher = tool("git.branch", vec![Effect::GitWrite]);
        assert!(matches!(
            policy.evaluate(&brancher, None),
            PolicyDecision::Deny { .. }
        ));
    }
}
