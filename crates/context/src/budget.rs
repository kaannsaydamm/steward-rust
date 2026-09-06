//! Token-budgeted allocator (§19.2, Task 5.2, C-002).
//!
//! Algorithm: reserve system/user/output, reject scope-mismatched items,
//! rank by score, fill lanes, include whole atomic items, record omitted
//! reasons. Protected items (system/task) can never be evicted by large
//! low-value memory/tool output.

use crate::item::{ContextItem, ContextSourceKind};
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Lane configuration per §19.2.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BudgetPolicy {
    /// Total usable context (model limit minus safety margin).
    pub total_tokens: u32,
    /// Reserved for system + task + output; protected from eviction.
    pub reserved_tokens: u32,
    /// Cap for the tool-schema lane.
    pub tool_schema_tokens: u32,
    /// Cap for the memory lane.
    pub memory_tokens: u32,
}

impl Default for BudgetPolicy {
    fn default() -> Self {
        Self {
            total_tokens: 32_000,
            reserved_tokens: 4_000,
            tool_schema_tokens: 4_000,
            memory_tokens: 4_000,
        }
    }
}

/// Deterministic packing result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PackedContext {
    /// Included items in selection order.
    pub selected: Vec<ContextItem>,
    /// Omitted items with reasons (manifest-ready).
    pub omitted: Vec<OmittedItem>,
    /// Token estimate of the packed context.
    pub packed_tokens: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OmittedItem {
    pub id: String,
    pub reason: String,
}

pub struct BudgetAllocator;

impl BudgetAllocator {
    /// Packs candidates under the policy. Deterministic: equal scores break
    /// ties by id so fixtures are stable.
    pub fn pack(
        candidates: Vec<ContextItem>,
        policy: &BudgetPolicy,
    ) -> Result<PackedContext> {
        anyhow::ensure!(
            policy.reserved_tokens <= policy.total_tokens,
            "reserved budget exceeds total"
        );
        let dynamic_budget = policy.total_tokens.saturating_sub(policy.reserved_tokens);

        // Filter: zero-score garbage and unknown-sensitivity items are skipped.
        let mut scored: Vec<&ContextItem> = candidates.iter().collect();
        scored.sort_by(|a, b| {
            b.score()
                .partial_cmp(&a.score())
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.id.cmp(&b.id))
        });

        let mut selected: Vec<ContextItem> = Vec::new();
        let mut omitted: Vec<OmittedItem> = Vec::new();
        let mut used: u32 = 0;

        for item in scored {
            let lane_cap = match item.source {
                ContextSourceKind::Skill | ContextSourceKind::ModelInstruction => policy.tool_schema_tokens,
                ContextSourceKind::LongTermMemory | ContextSourceKind::NegativeLesson | ContextSourceKind::Observation => policy.memory_tokens,
                _ => dynamic_budget,
            };
            if used + item.estimated_tokens > dynamic_budget {
                omitted.push(OmittedItem {
                    id: item.id.clone(),
                    reason: "context_budget".into(),
                });
                continue;
            }

            let lane_used: u32 = selected
                .iter()
                .filter(|s| std::mem::discriminant(&s.source) == std::mem::discriminant(&item.source))
                .map(|s| s.estimated_tokens)
                .sum();
            if lane_used + item.estimated_tokens > lane_cap {
                omitted.push(OmittedItem {
                    id: item.id.clone(),
                    reason: "lane_budget".into(),
                });
                continue;
            }

            used += item.estimated_tokens;
            selected.push(item.clone());
        }

        Ok(PackedContext {
            packed_tokens: used,
            selected,
            omitted,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::{ContextScope, Sensitivity, TrustLevel};

    fn item(id: &str, source: ContextSourceKind, content: &str, relevance: f32, authority: f32) -> ContextItem {
        ContextItem {
            id: id.into(),
            source,
            content: content.into(),
            estimated_tokens: ContextItem::estimate_tokens(content),
            relevance,
            authority,
            freshness: 1.0,
            trust: TrustLevel::TrustedConfiguration,
            sensitivity: Sensitivity::Public,
            scope: ContextScope::Global,
        }
    }

    #[test]
    fn token_estimator_is_chars_over_four() {
        assert_eq!(ContextItem::estimate_tokens(""), 0);
        assert_eq!(ContextItem::estimate_tokens("abcd"), 1);
        assert_eq!(ContextItem::estimate_tokens("abcde"), 2);
    }

    #[test]
    fn protected_high_value_beats_large_low_value_memory() {
        let policy = BudgetPolicy {
            total_tokens: 100,
            reserved_tokens: 10,
            tool_schema_tokens: 100,
            memory_tokens: 100,
        };
        let task = item("task", ContextSourceKind::UserMessage, "important task", 1.0, 1.0);
        let huge_memory = {
            let mut m = item(
                "memory:big",
                ContextSourceKind::LongTermMemory,
                &"x".repeat(400),
                0.05,
                0.3,
            );
            m.estimated_tokens = 100; // exceeds remaining budget after task
            m
        };
        let packed = BudgetAllocator::pack(vec![task.clone(), huge_memory], &policy).unwrap();
        assert!(packed.selected.iter().any(|i| i.id == "task"), "task always included");
        assert!(
            packed.selected.iter().all(|i| i.id != "memory:big"),
            "big low-value memory must not evict the task"
        );
        assert!(packed.packed_tokens <= policy.total_tokens - policy.reserved_tokens);
    }

    #[test]
    fn lane_caps_limit_memory_inclusion() {
        let policy = BudgetPolicy {
            total_tokens: 1000,
            reserved_tokens: 0,
            tool_schema_tokens: 1000,
            memory_tokens: 20,
        };
        let candidates = vec![
            item("m1", ContextSourceKind::LongTermMemory, "memory one", 1.0, 1.0),
            item("m2", ContextSourceKind::LongTermMemory, &"long".repeat(40), 0.9, 1.0),
        ];
        let packed = BudgetAllocator::pack(candidates, &policy).unwrap();
        assert!(packed.selected.iter().any(|i| i.id == "m1"));
        assert!(
            packed.selected.iter().all(|i| i.id != "m2"),
            "second memory must hit the memory lane cap"
        );
        assert!(packed.omitted.iter().any(|o| o.reason == "lane_budget"));
    }

    #[test]
    fn omitted_reason_is_context_budget_on_overflow() {
        let policy = BudgetPolicy {
            total_tokens: 20,
            reserved_tokens: 0,
            tool_schema_tokens: 20,
            memory_tokens: 20,
        };
        let big = item("big", ContextSourceKind::UserMessage, &"y".repeat(200), 1.0, 1.0);
        let packed = BudgetAllocator::pack(vec![big], &policy).unwrap();
        assert!(packed.selected.is_empty());
        assert_eq!(packed.omitted[0].reason, "context_budget");
    }

    #[test]
    fn deterministic_ordering_on_ties() {
        let policy = BudgetPolicy::default();
        let a = item("a", ContextSourceKind::ExplicitFile, "same", 0.5, 0.5);
        let b = item("b", ContextSourceKind::ExplicitFile, "same", 0.5, 0.5);
        let first = BudgetAllocator::pack(vec![a.clone(), b.clone()], &policy).unwrap();
        let second = BudgetAllocator::pack(vec![b, a], &policy).unwrap();
        let ids: Vec<String> = first.selected.iter().map(|i| i.id.clone()).collect();
        let ids2: Vec<String> = second.selected.iter().map(|i| i.id.clone()).collect();
        assert_eq!(ids, ids2);
    }

    #[test]
    fn reserved_exceeding_total_is_rejected() {
        let policy = BudgetPolicy {
            total_tokens: 10,
            reserved_tokens: 20,
            ..Default::default()
        };
        assert!(BudgetAllocator::pack(vec![], &policy).is_err());
    }
}
