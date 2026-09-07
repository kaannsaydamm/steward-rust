# Hermes upstream provenance

- Upstream repository: https://github.com/NousResearch/hermes-agent
- Pinned SHA (integration start): `693641aa8b4359c602283bdbbc14041e03bc47bc`
- License: MIT (© 2025 Nous Research) — vendored LICENSE files stay in-tree
- Imported paths: see docs/OSS_PORT_MAP.md (H1–H5)
- Steward target paths: `desktop/hermes/**`, `web-hermes/**`, `gateway/hermes/**`
- Files modified: tracked per-commit; minimal (branding, adapter hooks, build glue only)
- Integration boundary: gateway methods → Steward daemon gRPC/Wire v2
- Major local changes: listed as they land
- How to sync upstream:
  1. Fetch upstream at pinned SHA + target newer commit.
  2. Diff vendored trees vs upstream to get the local patch set.
  3. Re-apply patches on the new upstream tree, resolve conflicts.
  4. Run phase E2E green; update the log below.

## Upgraded SHA log

(none yet — refresh phase runs only after full integration green)
