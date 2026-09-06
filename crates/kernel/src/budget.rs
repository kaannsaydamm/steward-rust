//! Budgets instead of hard round limits (§17). Every dimension terminates a
//! run independently; child agents inherit explicit allocations.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Budget {
    pub max_model_calls: Option<u32>,
    pub max_tool_calls: Option<u32>,
    pub max_spawned_agents: Option<u32>,
    pub max_recursion_depth: Option<u16>,
    pub max_parallelism: Option<u16>,
    pub max_input_tokens: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub max_cost_microusd: Option<u64>,
    pub max_wall_clock_ms: Option<u64>,
    pub max_failures: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BudgetCheck {
    /// Under budget on every tracked dimension.
    Ok,
    /// A dimension is exhausted; the run must terminate.
    Exhausted(&'static str),
}

impl Budget {
    /// Checks consumption counts; `model_calls` counts every generation,
    /// `tool_calls` every executed tool, `failures` every error outcome.
    pub fn check(
        &self,
        model_calls: u32,
        tool_calls: u32,
        spawned_agents: u32,
        failures: u32,
        input_tokens: u64,
        output_tokens: u64,
        cost_microusd: u64,
        elapsed_ms: u64,
    ) -> BudgetCheck {
        if let Some(limit) = self.max_model_calls {
            if model_calls >= limit {
                return BudgetCheck::Exhausted("max_model_calls");
            }
        }
        if let Some(limit) = self.max_tool_calls {
            if tool_calls >= limit {
                return BudgetCheck::Exhausted("max_tool_calls");
            }
        }
        if let Some(limit) = self.max_spawned_agents {
            if spawned_agents >= limit {
                return BudgetCheck::Exhausted("max_spawned_agents");
            }
        }
        if let Some(limit) = self.max_failures {
            if failures >= limit {
                return BudgetCheck::Exhausted("max_failures");
            }
        }
        if let Some(limit) = self.max_input_tokens {
            if input_tokens >= limit {
                return BudgetCheck::Exhausted("max_input_tokens");
            }
        }
        if let Some(limit) = self.max_output_tokens {
            if output_tokens >= limit {
                return BudgetCheck::Exhausted("max_output_tokens");
            }
        }
        if let Some(limit) = self.max_cost_microusd {
            if cost_microusd >= limit {
                return BudgetCheck::Exhausted("max_cost_microusd");
            }
        }
        if let Some(limit) = self.max_wall_clock_ms {
            if elapsed_ms >= limit {
                return BudgetCheck::Exhausted("max_wall_clock_ms");
            }
        }
        BudgetCheck::Ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlimited_budget_always_ok() {
        let budget = Budget::default();
        assert_eq!(
            budget.check(
                1000,
                1000,
                1000,
                1000,
                u64::MAX,
                u64::MAX,
                u64::MAX,
                u64::MAX
            ),
            BudgetCheck::Ok
        );
    }

    #[test]
    fn model_call_budget_exhausts() {
        let budget = Budget {
            max_model_calls: Some(3),
            ..Default::default()
        };
        assert_eq!(budget.check(2, 0, 0, 0, 0, 0, 0, 0), BudgetCheck::Ok);
        assert_eq!(
            budget.check(3, 0, 0, 0, 0, 0, 0, 0),
            BudgetCheck::Exhausted("max_model_calls")
        );
    }

    #[test]
    fn each_dimension_terminates_independently() {
        let cases = [
            (
                Budget {
                    max_tool_calls: Some(1),
                    ..Default::default()
                },
                (0, 1, 0, 0),
                "max_tool_calls",
            ),
            (
                Budget {
                    max_spawned_agents: Some(1),
                    ..Default::default()
                },
                (0, 0, 1, 0),
                "max_spawned_agents",
            ),
            (
                Budget {
                    max_failures: Some(1),
                    ..Default::default()
                },
                (0, 0, 0, 1),
                "max_failures",
            ),
            (
                Budget {
                    max_input_tokens: Some(10),
                    ..Default::default()
                },
                (0, 0, 0, 0),
                "max_input_tokens",
            ),
            (
                Budget {
                    max_output_tokens: Some(10),
                    ..Default::default()
                },
                (0, 0, 0, 0),
                "max_output_tokens",
            ),
            (
                Budget {
                    max_cost_microusd: Some(10),
                    ..Default::default()
                },
                (0, 0, 0, 0),
                "max_cost_microusd",
            ),
            (
                Budget {
                    max_wall_clock_ms: Some(10),
                    ..Default::default()
                },
                (0, 0, 0, 0),
                "max_wall_clock_ms",
            ),
        ];
        for (budget, (m, t, s, f), expected) in cases {
            let (i, o) = if expected == "max_input_tokens" {
                (10u64, 0u64)
            } else if expected == "max_output_tokens" {
                (0, 10)
            } else {
                (0, 0)
            };
            let cost = if expected == "max_cost_microusd" {
                10u64
            } else {
                0u64
            };
            let elapsed = if expected == "max_wall_clock_ms" {
                10u64
            } else {
                0u64
            };
            assert_eq!(
                budget.check(m, t, s, f, i, o, cost, elapsed),
                BudgetCheck::Exhausted(expected),
                "dimension {expected}"
            );
        }
    }
}
