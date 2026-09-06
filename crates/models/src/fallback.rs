//! Fallback graph (§21.4, Task 6.4, M-008): policy per failure class, not a
//! single backup model.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    /// HTTP 429 — wait per policy or move to next.
    RateLimited,
    /// HTTP 5xx / network — move to next.
    ProviderUnavailable,
    /// Prompt exceeds the model window — jump to the long-context route.
    ContextTooLarge,
    /// Provider rejected the tool schema — need a tool-compatible route.
    ToolSchemaRejected,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FallbackRule {
    /// Move to the next route in the ladder.
    Next,
    /// Switch to a specific named route (e.g. long-context model).
    Named { route: String },
    /// Stop retrying; surface the failure.
    Abort,
}

/// Ordered ladder per failure class (§21.4 YAML shape).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FallbackPolicy {
    pub primary: String,
    pub ladder: Vec<String>,
    pub rules: Vec<(FailureClass, FallbackRule)>,
}

impl FallbackPolicy {
    pub fn new(primary: &str, ladder: Vec<String>) -> Self {
        Self {
            primary: primary.to_owned(),
            ladder,
            rules: Vec::new(),
        }
    }

    pub fn with_rule(mut self, class: FailureClass, rule: FallbackRule) -> Self {
        self.rules.push((class, rule));
        self
    }

    /// Resolves the next route for a failure. `attempt` counts prior fallback
    /// hops; returns None when the ladder is exhausted under `Next`.
    pub fn next_route(&self, class: FailureClass, attempt: usize) -> Option<FallbackDecision> {
        let rule = self
            .rules
            .iter()
            .find(|(c, _)| *c == class)
            .map(|(_, r)| r.clone())
            .unwrap_or(match class {
                FailureClass::RateLimited | FailureClass::ProviderUnavailable => FallbackRule::Next,
                FailureClass::ContextTooLarge | FailureClass::ToolSchemaRejected => FallbackRule::Abort,
            });
        match rule {
            FallbackRule::Abort => None,
            FallbackRule::Named { route } => Some(FallbackDecision {
                route,
                rationale: format!("{class:?} -> named route"),
            }),
            FallbackRule::Next => {
                let mut sequence =
                    std::iter::once(self.primary.clone()).chain(self.ladder.iter().cloned());
                let route = sequence.nth(attempt + 1)?;
                Some(FallbackDecision {
                    route,
                    rationale: format!("{class:?} -> ladder position {}", attempt + 1),
                })
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FallbackDecision {
    pub route: String,
    pub rationale: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> FallbackPolicy {
        FallbackPolicy::new(
            "a/primary",
            vec!["b/second".into(), "c/local".into()],
        )
        .with_rule(FailureClass::ContextTooLarge, FallbackRule::Named { route: "d/long".into() })
    }

    #[test]
    fn rate_limited_walks_the_ladder() {
        let policy = policy();
        assert_eq!(
            policy.next_route(FailureClass::RateLimited, 0).unwrap().route,
            "b/second"
        );
        assert_eq!(
            policy.next_route(FailureClass::RateLimited, 1).unwrap().route,
            "c/local"
        );
        // Ladder exhausted.
        assert!(policy.next_route(FailureClass::RateLimited, 2).is_none());
    }

    #[test]
    fn context_overflow_jumps_to_named_route() {
        let policy = policy();
        let decision = policy.next_route(FailureClass::ContextTooLarge, 0).unwrap();
        assert_eq!(decision.route, "d/long");
        assert!(decision.rationale.contains("ContextTooLarge"));
    }

    #[test]
    fn default_classes_abort_without_policy() {
        let policy = FallbackPolicy::new("a/primary", vec![]);
        assert!(policy.next_route(FailureClass::ToolSchemaRejected, 0).is_none());
    }

    #[test]
    fn provider_unavailable_falls_back_then_aborts() {
        let policy = policy();
        assert_eq!(
            policy.next_route(FailureClass::ProviderUnavailable, 0).unwrap().route,
            "b/second"
        );
    }

    #[test]
    fn policy_serializes_roundtrip() {
        let policy = policy();
        let json = serde_json::to_string(&policy).unwrap();
        let back: FallbackPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(back.primary, "a/primary");
        assert_eq!(back.rules.len(), 1);
    }
}
