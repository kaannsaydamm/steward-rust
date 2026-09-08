# AUTOGEN upstream provenance

- Upstream repository: https://github.com/microsoft/autogen
- Pinned SHA (integration start): `027ecf0a379bcc1d09956d46d12d44a3ad9cee14`
- License: code MIT via LICENSE-CODE (© Microsoft Corporation); docs/prose CC-BY-4.0 (NOT vendored)
- Imported paths: see docs/OSS_PORT_MAP.md (A1–A4)
- Steward target paths: `builder/autogen-studio/**`
- Files modified: REST shim (studio frontend → Steward SaveAgentProfile RPCs), schema mapping to AgentProfile YAML
- Integration boundary: builder config → AgentProfile → kernel execution; team configs → Team* IR nodes
- How to sync upstream: same procedure as docs/upstream/HERMES.md

## Upgraded SHA log

- Upgraded: `027ecf0a379bcc1d09956d46d12d44a3ad9cee14` (2026-09-08) — no-op refresh: upstream
  `main` HEAD still equals the pinned SHA (AutoGen is in maintenance mode since 2026-04; successor
  is microsoft/agent-framework). Vendored `autogen-core/`, `autogen-agentchat/`, `autogen-studio/`
  verified byte-identical to upstream `python/packages/*` at HEAD; no layout delta (autogen-studio
  unchanged at `python/packages/autogen-studio`), builder shim semantics untouched.
