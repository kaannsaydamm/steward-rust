# OH_MY_PI upstream provenance

- Upstream repository: https://github.com/can1357/oh-my-pi
- Pinned SHA (integration start): `a1b254047d12e143b7c6011536e918c6c35c5906`
- License: MIT (© 2025 Mario Zechner)
- Imported paths: see docs/OSS_PORT_MAP.md (O1–O5)
- Steward target paths: `cli-omp/packages/**`
- Files modified: minimal — provider/agent-loop boundary adapter, branding
- Integration boundary: Steward provider facade over daemon Chat streaming; sessions mapped to Steward session ids
- How to sync upstream: same procedure as docs/upstream/HERMES.md

## Upgraded SHA log

- Upgraded: fb16d5a6c73499ecf0035dc1953c6680d35f562d (2026-09-08) — coding-agent 18.1.13→18.1.14; tool-views.generated.js regenerated; pi_natives.win32-x64-modern.node preserved from pin (not present at new SHA)
