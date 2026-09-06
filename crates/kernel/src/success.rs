//! Success and verification (§18): a run terminates on explicit criteria,
//! never merely on "rounds ran out".

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SuccessPolicy {
    /// The agent itself declares completion (chat default).
    AgentDeclared,
    /// The user accepts the result manually.
    UserAccepted,
    /// Every criterion must hold.
    All(Vec<SuccessCriterion>),
    /// Any criterion suffices.
    Any(Vec<SuccessCriterion>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SuccessCriterion {
    CommandExitZero { command: String },
    FileExists { path: String },
    ArtifactProduced { kind: String },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SuccessVerdict {
    Satisfied,
    Unsatisfied,
    Pending,
}

/// Evaluates the policy against observed run facts.
#[async_trait]
pub trait SuccessEvaluator: Send + Sync {
    async fn evaluate(&self, policy: &SuccessPolicy, last_text: &str) -> Result<SuccessVerdict>;
}

/// Default evaluator: `AgentDeclared` completes on a Final action;
/// `All`/`Any` delegate each criterion to the provided checker closures.
pub struct PolicyEvaluator<F>
where
    F: Fn(&SuccessCriterion) -> Result<bool> + Send + Sync,
{
    pub check: F,
}

#[async_trait]
impl<F> SuccessEvaluator for PolicyEvaluator<F>
where
    F: Fn(&SuccessCriterion) -> Result<bool> + Send + Sync,
{
    async fn evaluate(&self, policy: &SuccessPolicy, _last_text: &str) -> Result<SuccessVerdict> {
        match policy {
            SuccessPolicy::AgentDeclared | SuccessPolicy::UserAccepted => Ok(SuccessVerdict::Satisfied),
            SuccessPolicy::All(criteria) => {
                for criterion in criteria {
                    if !(self.check)(criterion)? {
                        return Ok(SuccessVerdict::Unsatisfied);
                    }
                }
                Ok(SuccessVerdict::Satisfied)
            }
            SuccessPolicy::Any(criteria) => {
                if criteria.is_empty() {
                    return Ok(SuccessVerdict::Unsatisfied);
                }
                for criterion in criteria {
                    if (self.check)(criterion)? {
                        return Ok(SuccessVerdict::Satisfied);
                    }
                }
                Ok(SuccessVerdict::Unsatisfied)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker(criterion: &SuccessCriterion) -> Result<bool> {
        match criterion {
            SuccessCriterion::FileExists { path } => Ok(path == "exists.txt"),
            _ => Ok(false),
        }
    }

    fn evaluator() -> PolicyEvaluator<fn(&SuccessCriterion) -> Result<bool>> {
        PolicyEvaluator { check: checker }
    }

    #[tokio::test]
    async fn agent_declared_always_satisfied() {
        let verdict = evaluator()
            .evaluate(&SuccessPolicy::AgentDeclared, "done")
            .await
            .unwrap();
        assert_eq!(verdict, SuccessVerdict::Satisfied);
    }

    #[tokio::test]
    async fn all_requires_every_criterion() {
        let policy = SuccessPolicy::All(vec![
            SuccessCriterion::FileExists { path: "exists.txt".into() },
            SuccessCriterion::FileExists { path: "missing.txt".into() },
        ]);
        assert_eq!(
            evaluator().evaluate(&policy, "").await.unwrap(),
            SuccessVerdict::Unsatisfied
        );

        let policy = SuccessPolicy::All(vec![SuccessCriterion::FileExists {
            path: "exists.txt".into(),
        }]);
        assert_eq!(
            evaluator().evaluate(&policy, "").await.unwrap(),
            SuccessVerdict::Satisfied
        );
    }

    #[tokio::test]
    async fn any_satisfies_on_first_match() {
        let policy = SuccessPolicy::Any(vec![
            SuccessCriterion::FileExists { path: "missing.txt".into() },
            SuccessCriterion::FileExists { path: "exists.txt".into() },
        ]);
        assert_eq!(
            evaluator().evaluate(&policy, "").await.unwrap(),
            SuccessVerdict::Satisfied
        );
    }

    #[tokio::test]
    async fn empty_any_is_unsatisfied() {
        let policy = SuccessPolicy::Any(vec![]);
        assert_eq!(
            evaluator().evaluate(&policy, "").await.unwrap(),
            SuccessVerdict::Unsatisfied
        );
    }
}
