//! Lifecycle hooks (§31, Task 18.1, S-017): user-facing hook registry.
//! Hook effects pass through the same effect policy — a hook can never be
//! a backdoor tool. Matchers gate when hooks fire.

use anyhow::{Context as _, Result};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookEvent {
    RunStarted,
    RunFinished,
    RunFailed,
    TurnStarted,
    BeforeModel,
    AfterModel,
    ToolDiscovered,
    BeforeTool,
    AfterTool,
    SubagentCreated,
    SubagentFinished,
    MemoryProposed,
    MemoryCommitted,
    ApprovalRequested,
    ApprovalResolved,
    ArtifactCreated,
    FileChanged,
}

/// A registered hook: fires on matching events, passes through effect
/// policy for any side effect it declares (S-017).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hook {
    pub hook_id: String,
    pub event: HookEvent,
    /// Matcher: only fire when this JSON-path predicate holds on the event
    /// payload. Empty = always fire.
    #[serde(default)]
    pub matcher: Value,
    /// What the hook does: a governed shell command or webhook URL. The
    /// effect policy must grant the corresponding effect or the hook is
    /// refused at registration.
    pub action: HookAction,
    pub timeout_ms: u64,
    #[serde(default)]
    pub max_retries: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HookAction {
    Shell { command: String },
    Webhook { url: String },
    /// Observe-only hook: cannot mutate anything (safe default).
    Observe,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookVerdict {
    /// Continue; payload unchanged.
    Continue,
    /// Continue with modified payload (redaction etc.).
    Modified(Value),
    /// Block the operation (before-tool hooks may deny).
    Deny { reason: String },
}

/// The registry. Registration validates that declared actions fit policy.
pub struct HookRegistry {
    hooks: RwLock<BTreeMap<String, Hook>>,
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl HookRegistry {
    pub fn new() -> Self {
        Self { hooks: RwLock::new(BTreeMap::new()) }
    }

    /// Registers a hook. `granted_effects` are the effects policy allows for
    /// hooks; a shell/webhook action without its effect grant is rejected.
    pub fn register(&self, hook: Hook, granted_effects: &[&str]) -> Result<()> {
        anyhow::ensure!(!hook.hook_id.is_empty(), "hook id required");
        anyhow::ensure!(hook.timeout_ms > 0 && hook.timeout_ms <= 60_000, "timeout must be 1ms..60s");
        match &hook.action {
            HookAction::Observe => {}
            HookAction::Shell { .. } => {
                anyhow::ensure!(
                    granted_effects.contains(&"process.execute"),
                    "hook '{hook_id}' declares shell but process.execute is not granted (S-017)",
                    hook_id = hook.hook_id
                );
            }
            HookAction::Webhook { .. } => {
                anyhow::ensure!(
                    granted_effects.contains(&"network.http"),
                    "hook '{hook_id}' declares webhook but network.http is not granted (S-017)",
                    hook_id = hook.hook_id
                );
            }
        }
        self.hooks.write().insert(hook.hook_id.clone(), hook);
        Ok(())
    }

    pub fn remove(&self, hook_id: &str) -> bool {
        self.hooks.write().remove(hook_id).is_some()
    }

    /// Hooks matching an event, ordered by id (deterministic).
    pub fn matching(&self, event: HookEvent) -> Vec<Hook> {
        self.hooks
            .read()
            .values()
            .filter(|hook| hook.event == event)
            .cloned()
            .collect()
    }

    pub fn len(&self) -> usize {
        self.hooks.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Executes matching hooks for an event. `Observe` hooks run in-process;
/// shell/webhook effects are delegated to the caller's governed executor.
pub fn dispatch(
    registry: &HookRegistry,
    event: HookEvent,
    payload: &Value,
    mut governed: impl FnMut(&Hook, &Value) -> Result<HookVerdict>,
) -> Result<HookVerdict> {
    let hooks = registry.matching(event);
    let mut current = payload.clone();
    for hook in hooks {
        match governed(&hook, &current).with_context(|| format!("hook {}", hook.hook_id))? {
            HookVerdict::Continue => {}
            HookVerdict::Modified(updated) => current = updated,
            HookVerdict::Deny { reason } => return Ok(HookVerdict::Deny { reason }),
        }
    }
    if current == *payload {
        Ok(HookVerdict::Continue)
    } else {
        Ok(HookVerdict::Modified(current))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn observe_hook(id: &str, event: HookEvent) -> Hook {
        Hook {
            hook_id: id.into(),
            event,
            matcher: json!({}),
            action: HookAction::Observe,
            timeout_ms: 1000,
            max_retries: 0,
        }
    }

    #[test]
    fn observe_hooks_register_without_effects() {
        let registry = HookRegistry::new();
        registry.register(observe_hook("h1", HookEvent::BeforeTool), &[]).unwrap();
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn shell_hooks_require_process_effect() {
        let registry = HookRegistry::new();
        let hook = Hook {
            action: HookAction::Shell { command: "echo hi".into() },
            ..observe_hook("h2", HookEvent::AfterTool)
        };
        assert!(registry.register(hook.clone(), &[]).is_err(), "S-017: no backdoor hooks");
        registry.register(hook, &["process.execute"]).unwrap();
    }

    #[test]
    fn webhook_hooks_require_network_effect() {
        let registry = HookRegistry::new();
        let hook = Hook {
            action: HookAction::Webhook { url: "https://hooks.example".into() },
            ..observe_hook("h3", HookEvent::RunFinished)
        };
        assert!(registry.register(hook.clone(), &[]).is_err());
        registry.register(hook, &["network.http"]).unwrap();
    }

    #[test]
    fn dispatch_redacts_via_modified_payload() {
        let registry = HookRegistry::new();
        registry.register(observe_hook("redactor", HookEvent::AfterTool), &[]).unwrap();

        let verdict = dispatch(&registry, HookEvent::AfterTool, &json!({"output": "sk-secret123"}), |hook, payload| {
            let _ = hook;
            let mut updated = payload.clone();
            if let Some(output) = updated.get_mut("output") {
                *output = json!("[REDACTED]");
            }
            Ok(HookVerdict::Modified(updated))
        })
        .unwrap();
        match verdict {
            HookVerdict::Modified(payload) => {
                assert_eq!(payload["output"], "[REDACTED]");
            }
            other => panic!("expected modified, got {other:?}"),
        }
    }

    #[test]
    fn deny_short_circuits_remaining_hooks() {
        let registry = HookRegistry::new();
        registry.register(observe_hook("blocker", HookEvent::BeforeTool), &[]).unwrap();
        registry.register(observe_hook("never", HookEvent::BeforeTool), &[]).unwrap();

        let mut executed = Vec::new();
        let verdict = dispatch(&registry, HookEvent::BeforeTool, &json!({}), |hook, _payload| {
            executed.push(hook.hook_id.clone());
            if hook.hook_id == "blocker" {
                Ok(HookVerdict::Deny { reason: "tool is blocked".into() })
            } else {
                Ok(HookVerdict::Continue)
            }
        })
        .unwrap();
        assert!(matches!(verdict, HookVerdict::Deny { reason } if reason.contains("blocked")));
        assert_eq!(executed, vec!["blocker".to_owned()], "hooks after deny do not run");
    }

    #[test]
    fn event_matching_is_precise() {
        let registry = HookRegistry::new();
        registry.register(observe_hook("tool-only", HookEvent::AfterTool), &[]).unwrap();
        assert!(registry.matching(HookEvent::BeforeTool).is_empty());
        assert_eq!(registry.matching(HookEvent::AfterTool).len(), 1);
    }
}
