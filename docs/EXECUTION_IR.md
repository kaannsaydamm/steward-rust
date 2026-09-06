# Execution IR (Omega §12)

The visual workflow editor, declarative YAML/JSON, autonomous agent
planning, and RLM orchestration all compile to one serialized plan:
`steward_kernel::ir::ExecutionPlan` (`schema_version: 1`).

## Node kinds

`agent` (profile + task), `tool`, `code`, `native`, `human`, `verify`,
`router`, `subworkflow`.

## Edge kinds

| Kind | Semantics |
|---|---|
| `direct` | B ready when A completes (G-008) |
| `conditional` | predicate over node outputs (safe evaluator: `==`, `!=`, `contains`; quoted literals vs field lookup; arbitrary code rejected — G-009) |
| `fan_out` | branches start in parallel within `max_parallelism` (G-010) |
| `fan_in` | join policy `all` / `any` / `quorum{n}` (G-011) |
| `retry` | max attempts + backoff ms; persists across restarts (G-014) |
| `fallback` | terminal failure routes to fallback path (G-015) |

## Validation

`ExecutionPlan::validate` rejects: unknown edge endpoints, self-loops,
cycles, fan-in without parallel inputs, unreachable nodes, bad schema
versions.

## Scheduler semantics (§13)

Partial superstep recovery: completed nodes never re-run after a sibling
failure (G-016). Upstream outputs are passed to downstream executors via
state. Results are keyed by node id, deterministic ordering.

## v1 migration

`ExecutionPlan::from_v1_workflow` compiles legacy JSON DAGs to equivalent
sequential plans (direct edges, single entry) — Task 12.5.

## Runtime manager contract (Task 14.5)

`runtimes/` are version-pinned, checksum-verified packages (`RUNTIME_SPECS`).
Install is atomic with rollback; offline failures are clear errors. No
arbitrary `curl | sh` anywhere in the agent (S-018).
