# PRIME_AGENT upstream provenance

- Upstream repository: https://github.com/PrimeIntellect-ai/prime-agent
- Pinned SHA (integration start): `844e85545af6858dcb3d6cfe42bbfcf2ca0be4e5`
- License: MIT (© 2025 Mario Zechner)
- Imported paths: see docs/OSS_PORT_MAP.md (P1–P3)
- Steward target paths: `runtimes/prime-rlm/**`
- Files modified: sidecar spawn/shutdown glue in `crates/kernel` (extension, not rewrite)
- Integration boundary: `AgentAction::RlmRun` → Prime REPL substrate (NDJSON stdio protocol v3)
- How to sync upstream: same procedure as docs/upstream/HERMES.md

## Upgraded SHA log

- Upgraded: `bf8894afa55832f7cfa2094c8a0d041bc680a691` (2026-09-08) — reference refresh of
  `vendor/prime/prime-agent-runtime/**` only (REPL cell-completion barrier, bash/interrupt hardening,
  new `test_create_session.py`). Upstream restructured `packages/coding-agent/src/...`; the
  Steward-local flattened mirror `vendor/prime/orchestrator/` (↔ `packages/coding-agent/src/core/`
  + `src/modes/daemon/` at the pinned SHA) is preserved unchanged; upstream moved `rlm-runtime.ts`
  within `packages/coding-agent/src/core/` (content changed) — port unaffected.
