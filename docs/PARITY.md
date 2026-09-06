# Parity Registry

Capability-class parity targets from the Omega master plan (Part III) with
their burn-down status at the end of the Omega implementation.

Statuses are **not hand-written as green**. Each entry links the enforcing
component and the test evidence; suites live in the workspace and run via
`cargo test --workspace`.

## Burn-down (end of Omega implementation)

| Id | Capability class | Status | Evidence |
|---|---|---|---|
| K-001 | Small hot loop | GREEN | `kernel::agent::tests` (12 scenarios, no fixed round limit; emergency ceiling configurable) |
| K-002 | Structured tool agent mode | GREEN | `tool_action_executes_once_and_enters_history_canonically` |
| K-005 | Recursive agent spawn | GREEN | `recursion_limit_terminates_safely` (depth gate) |
| K-007/K-008 | Async/detached subagents | GREEN | `agents/task.rs` registry + restart reconciliation |
| K-009 | Agent messaging | GREEN | `mailbox_is_ordered_with_provenance` |
| K-010 | Declarative AgentProfile | GREEN | `parses_yaml_profile_without_code` (no Rust for new profiles) |
| K-012 | Goal-directed termination | GREEN | `success::tests` (All/Any/AgentDeclared/UserAccepted) |
| K-013/K-014 | Steer + wake signals | GREEN | `kernel::signals` (idempotent Wake, second-client steering) |
| K-015 | Human-in-the-loop | GREEN | `kernel::hitl` (durable pause, provenanced resolve) |
| K-016 | Budget manager | GREEN | `budget::tests` (each dimension terminates independently) |
| K-017 | Cancellation propagation | GREEN | `cancellation_propagates_to_descendants` + process cancel |
| G-001 | Unified Execution IR | GREEN | `plan_roundtrips_through_json`, validator suite |
| G-009/G-010/G-011 | Conditional/fan-out/fan-in | GREEN | `ir::scheduler::tests` (concurrency by wall-clock, quorum joins) |
| G-016 | Partial superstep recovery | GREEN | `failed_branch_does_not_rerun_successful_siblings` |
| G-017 | Breakpoints | GREEN | `kernel::hitl` breakpoint set |
| M-004/M-006 | Custom endpoints + capability metadata | GREEN | `models::catalog` (user registration without code) |
| M-007 | Task-aware routing | GREEN | `capability_constraints_beat_soft_score`, `local_only_never_routes_to_remote` |
| M-008 | Fallback ladder | GREEN | `fallback::tests` (rate-limit ladder, named overflow route) |
| S-001 | Effect-based authorization | GREEN | `aliasing_tool_names_cannot_bypass_effects` |
| S-002/S-003 | Scoped approvals + provenance | GREEN | `tools::approval` (once/run/workspace/effect+path, expiry, who/when/digest) |
| S-005 | Secret-minimized process env | GREEN | `spawn` env_clear + allowlist |
| S-012 | Secret OS vault | GREEN | `os_store_roundtrips_through_credential_manager` + live providers.json check |
| S-017 | Hook sandbox | GREEN | `shell_hooks_require_process_effect` (no backdoor hooks) |
| C-001/C-002 | Context manifest + budget packing | GREEN | `context::budget::tests` (protected lanes, omitted reasons) |
| C-005 | Path-scoped rules | GREEN | `selector_loads_matching_only` |
| C-006 | Progressive skill loading | GREEN | `skills` levels 1/2/3 tests |
| C-008/C-011 | Repo map / memory selection | GREEN | `repomap::tests`, `memory::tests` |
| C-010/C-017 | Observations + provenance | GREEN | `compaction::tests` |
| C-015 | Git-backed context repo | GREEN | `context_repo::tests` (commit/diff/rollback/branch) |
| C-020 | Dream consolidation | GREEN | `dream_dedupes_repeated_failure_patterns` |
| D-001 | Unified read | GREEN | `reader::tests` (text/json/dir/zip/sqlite-meta/binary-meta) |
| D-002/D-003 | Hash-anchored + atomic edits | GREEN | `edit::tests` (stale revision/line rejected, bottom-up batch) |
| D-006..D-011 | LSP surface | GREEN | `lsp_tests` mock conformance (8 cases) |
| D-012/D-013 | DAP surface | GREEN | `dap_tests` (policy-gated launch, breakpoints, variables) |
| D-014/D-015 | Processes + worktrees | GREEN | `workspace::tests` (ring buffer, cancel, isolated diffs, safe cleanup) |
| D-017 | Read-only explorer | GREEN | `explorer_cannot_write_even_if_model_requests` |
| S-014 | Remote HTTPS guard | GREEN | profile validation + live evals |
| P-006/P-008 | Wire v2 (native + WS) | GREEN | `wire` envelope roundtrips + `app_server` origin/auth tests |
| M-001..M-003 | Provider protocols (live) | GREEN | `tests/parity/v1_live.rs` 4/4 live evals (OpenAI-compatible) |
| U-003/U-005 | Attach/steer cross-client | GREEN | SignalBus + v1_live session persistence |

## Known deferrals (documented, not silently missing)

| Id | Capability | Status | Reason |
|---|---|---|---|
| K-003/K-004 | Code/RLM mode from model actions | FOUNDATION | Sidecar protocol + Python runtime live; model-action → sidecar bridge is the next slice (§27.3) |
| D-004/D-005 | Tree-sitter AST search/rewrite | FOUNDATION | Heuristic symbol graph ships; tree-sitter backends swap in behind the same types (§25.3) |
| G-013 | Dynamic graph mutation | FOUNDATION | IR supports ephemeral nodes; planner-side UI badge pending |
| G-018/G-019 | Durable timers / event-triggered continuation | FOUNDATION | cron v2 paths exist; timer persistence needs the scheduler store |
| S-006/S-007 | OCI/SSH sandbox backends | FOUNDATION | Backend traits + local/WASM complete; conformance requires Docker/SSH host |
| P-004/P-005 | ACP/A2A adapters | FOUNDATION | Protocol adapters land after Wire binary codec |
| 11.x | `steward-evals parity verify` CLI | FOUNDATION | Registry is the source of truth; verify command wraps `cargo test` |
| Release | Cross-platform packaging (macOS/Linux artifacts, winget, brew) | FOUNDATION | Windows verified; CI runner quota blocked hosted verification |

A deferral is an explicit engineering decision recorded here — not a claim
of full parity. "Full parity" is declared only when this table is empty of
`FOUNDATION` rows.
