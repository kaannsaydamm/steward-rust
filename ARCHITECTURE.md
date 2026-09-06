# Steward Omega — Architecture

Steward is a local-first **agent operating kernel** with one daemon and many
clients (CLI/TUI, Web, Desktop). The agent hot loop is small; everything else
lives in composable services.

## One architectural rule (Appendix E)

```text
ONE KERNEL, ONE DURABLE STATE MODEL, ONE EVENT MODEL,
ONE POLICY MODEL, ONE EXECUTION IR
MANY CLIENTS, MODELS, TOOLS, AGENTS, WORKFLOWS
```

## Workspace map

```text
crates/
├── core/       stable domain fundamentals: typed IDs, storage, provider
│               config (secret refs!), SecretStore, proto bindings
├── kernel/     execution semantics: TurnEngine, budgets, success,
│               Execution IR + concurrent scheduler, HITL interrupts,
│               signals, trace spans, RLM sidecar protocol
├── evals/      deterministic ScriptedModel + parity registry
├── context/    context engineering: candidates, token-budget allocator,
│               manifests, path-scoped rules, observational compaction
├── models/     capability catalog, task-aware router, fallback ladders,
│               provider health
├── tools/      ToolSpec, effect-based policy, scoped approvals, lazy
│               discovery, parallel batch execution
├── workspace/  leases (traversal-proof), process manager, git worktrees
├── coding/     repo index, unified reader, hash-anchored edits, symbol
│               graph, LSP + DAP managers
├── harness/    AgentProfiles, subagent scheduler + mailbox, git-backed
│               context repo, memory v2, skills, dream/refine, hooks
├── wire/       transport-independent v2 envelopes + handshake
├── daemon/     composition root: gRPC v1 (compat), Wire v2 WebSocket,
│               DB migrator + durable stores, kernel adapter
└── cli/        TUI + commands (attach to the same daemon state)
runtimes/python/  managed sidecar speaking the sidecar wire
web-ui/           static-export Next.js (shared by Web + Desktop)
```

## Execution paths

- **Chat**: `agent_runtime::run` → v1 loop (default) or
  `kernel_adapter` → `TurnEngine` (`STEWARD_AGENT_RUNTIME=omega`).
- **Workflows**: legacy DAGs compile via `ExecutionPlan::from_v1_workflow`;
  new plans run on the concurrent scheduler (fan-out/join/conditions).
- **RLM**: `code_runtime` sessions (Python sidecar live, Bun wire-compatible);
  `agents.map` compiles to scheduler-visible IR, never hidden concurrency.

## Durability

SQLite (WAL) via versioned migrations with pre-migration backups; ordered
per-run events; checkpoints with parent lineage; fork/replay; run state
machine (`pending→running→succeeded/failed/cancelled/unknown_effect`).

## Security

See `docs/SECURITY_ARCHITECTURE.md` — effects are the permission, secrets
live in the OS vault, untrusted text is never policy, hooks are not
backdoors.
