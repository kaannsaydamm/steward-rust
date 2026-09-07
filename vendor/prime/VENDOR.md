# Vendored: PrimeIntellect-ai/prime-agent (RLM subsystem)

- Source: https://github.com/PrimeIntellect-ai/prime-agent
- Pinned SHA: 844e85545af6858dcb3d6cfe42bbfcf2ca0be4e5
- License: MIT (© 2025 Mario Zechner) — see LICENSE
- Vendored paths:
  - `prime-agent-runtime/` — the real RLM substrate (persistent CPython REPL over NDJSON
    stdio, protocol v3; snapshots; bash; MCP; Windows job objects)
  - `orchestrator/rlm-runtime.ts`, `orchestrator/rlm-max-depth.ts` — run request / subagent
    registry / recursion depth semantics (upstream TS, consumed by the Steward kernel adapter)
  - `orchestrator/daemon/` — upstream daemon-mode ledger + subagent display
- Local modifications: none yet; integration glue lives outside this tree
- Sync: mirrors upstream paths at the pinned SHA
