//! Parity capability registry: every capability class from the Omega plan
//! (Part III) gets a stable id, its source harness families, and the required
//! test owners. Status is not hand-written as green; verification tooling
//! derives it from the referenced suites.

/// One parity capability line.
#[derive(Clone, Copy, Debug)]
pub struct ParityCapability {
    /// Stable id, e.g. "K-005".
    pub id: &'static str,
    /// Reference harness families that exhibit this capability.
    pub source_families: &'static [&'static str],
    /// Tests that must be green to claim the capability.
    pub required_tests: &'static [&'static str],
}

macro_rules! capability {
    ($id:expr, [$($family:expr),*], [$($test:expr),*]) => {
        ParityCapability {
            id: $id,
            source_families: &[$($family),*],
            required_tests: &[$($test),*],
        }
    };
}

/// The Phase 0 registry seed. New capabilities are appended as their tests land;
/// entries are removed only by a documented decision record.
pub const PARITY_REGISTRY: &[ParityCapability] = &[
    // Kernel and agent runtime
    capability!(
        "K-001",
        ["mini-swe-agent", "deep-agents"],
        ["kernel::agent::tests::completes_without_fixed_round_limit"]
    ),
    capability!(
        "K-002",
        ["claude-code", "codex", "smolagents"],
        ["kernel::agent::tests::tool_action_single_observation"]
    ),
    capability!(
        "K-005",
        ["prime-agent"],
        ["kernel::agents::tests::recursive_spawn_budget"]
    ),
    capability!(
        "K-012",
        ["agno"],
        ["kernel::agent::tests::success_policy_terminates_run"]
    ),
    capability!(
        "K-015",
        ["langgraph", "msft-agent-framework"],
        ["kernel::agent::tests::approval_pauses_then_resumes"]
    ),
    capability!(
        "K-016",
        ["modern-harness"],
        ["kernel::agent::tests::budget_exhaustion_stops_run"]
    ),
    capability!(
        "K-017",
        ["steward-existing"],
        ["kernel::agent::tests::cancellation"]
    ),
    // Workflows / Execution IR
    capability!(
        "G-001",
        ["google-adk"],
        ["kernel::ir::tests::plan_roundtrips"]
    ),
    capability!(
        "G-009",
        ["msft-agent-framework", "langgraph"],
        ["kernel::ir::tests::conditional_edges_route"]
    ),
    capability!(
        "G-010",
        ["msft-agent-framework", "langgraph"],
        ["kernel::scheduler::tests::fan_out_runs_concurrently"]
    ),
    capability!(
        "G-011",
        ["msft-agent-framework", "langgraph"],
        ["kernel::scheduler::tests::join_waits_for_all"]
    ),
    // Model layer
    capability!(
        "M-007",
        ["steward"],
        ["models::router::tests::capability_constraints_beat_soft_score"]
    ),
    capability!(
        "M-008",
        ["gemini-cli"],
        ["models::fallback::tests::rate_limited_falls_back"]
    ),
    // Tools/policy/security
    capability!(
        "S-001",
        ["steward-original"],
        ["tools::policy::tests::alias_cannot_bypass_effects"]
    ),
    capability!(
        "S-002",
        ["claude-code", "codex"],
        ["tools::approval::tests::scoped_approval_paths"]
    ),
    capability!(
        "S-012",
        ["security-requirement"],
        ["core::secrets::tests::secret_never_in_providers_json"]
    ),
    // Context
    capability!(
        "C-001",
        ["cursor", "prime-agent"],
        ["context::budget::tests::protected_items_survive"]
    ),
    capability!(
        "C-005",
        ["claude-code"],
        ["context::rules::tests::rule_loads_only_on_match"]
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_ids_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for capability in PARITY_REGISTRY {
            assert!(
                seen.insert(capability.id),
                "duplicate parity id {}",
                capability.id
            );
        }
    }

    #[test]
    fn every_capability_has_required_tests() {
        for capability in PARITY_REGISTRY {
            assert!(
                !capability.required_tests.is_empty(),
                "{} has no tests",
                capability.id
            );
            assert!(
                !capability.source_families.is_empty(),
                "{} has no sources",
                capability.id
            );
        }
    }
}
