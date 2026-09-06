# Parity Registry

Capability-class parity targets from the Omega master plan (Part III). Each
entry lists the reference harness families whose capability class Steward
reproduces and the tests that must be green before the capability is claimed.

Statuses are **not hand-written**. `steward-evals` derives them from the
referenced suites; a capability without a green referenced test is
`NOT_STARTED`/`FOUNDATION` regardless of prose claims.

| Id | Capability class | Reference families | Steward owner | Required tests |
|---|---|---|---|---|
| K-001 | Small hot loop | mini-SWE-agent, Deep Agents | `crates/kernel` | kernel turn loop tests |
| K-002 | Structured tool agent mode | Claude Code, Codex, smolagents | `kernel::turn` | single canonical tool observation |
| K-005 | Recursive agent spawn | Prime Agent | `kernel::subagents` | recursion budget termination |
| K-012 | Goal-directed termination | Agno | `kernel::termination` | success policy tests |
| K-015 | Human-in-the-loop | LangGraph, MS Agent Framework | `kernel::interrupts` | approval pause/resume |
| K-016 | Budget manager | modern harnesses | `kernel::budget` | per-budget termination |
| K-017 | Run cancellation | Steward existing | `kernel::control` | cancellation propagation |
| G-001 | Unified Execution IR | Google ADK | `kernel::ir` | plan roundtrip |
| G-009 | Conditional edges | MS Agent Framework, LangGraph | `kernel::scheduler` | predicate routing |
| G-010 | Fan-out | MS Agent Framework, LangGraph | `kernel::scheduler` | concurrent branches |
| G-011 | Fan-in / join | MS Agent Framework, LangGraph | `kernel::scheduler` | join policies |
| M-007 | Task-aware routing | Steward | `crates/models` | capability constraints beat soft score |
| M-008 | Fallback ladder | Gemini CLI | `models::fallback` | rate-limit fallback |
| S-001 | Effect-based authorization | Steward original | `tools::policy` | alias cannot bypass effects |
| S-002 | Scoped approval | Claude Code, Codex | `tools::approval` | scope separation |
| S-012 | Secret OS vault | security requirement | `core::secrets` | key never in providers.json |
| C-001 | Context as build product | Cursor, Prime Agent | `crates/context` | manifest per generation |
| C-005 | Path-scoped rules | Claude Code | `context::rules` | load only on match |

## Baseline (Phase 0) status

- `crates/evals`: deterministic `ScriptedModel` harness + parity registry seed — green.
- `tests/parity/v1_live.rs`: v1 agent loop verified against the live OpenAI-compatible
  provider (plain completion, session persistence, tool roundtrip) — green when
  `STEWARD_TEST_BASE_URL`/`STEWARD_TEST_API_KEY` are set; skipped otherwise.
- `tests/harness.rs::unused_port` skips Windows Hyper-V/WinNAT excluded port
  ranges (OS error 10013) — daemon spawn is deterministic on Windows again.
