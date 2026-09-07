# OSS Port Map — Source → Steward Target

Reuse types: `LITERAL` (verbatim file copy) · `VENDORED` (copied tree, minimal build glue) ·
`MINIMALLY_MODIFIED` (small patches: branding, paths, adapter hooks) · `ADAPTED` (new glue
code wrapping upstream contract) · `REIMPLEMENTED` (exception — must be justified per item).

Start SHAs: hermes `693641a`, omp `a1b2540`, prime `844e855`, autogen `027ecf0`.
Target upstream sync strategy: preserve upstream directory structure inside vendor trees so
future `git subtree`-style refresh stays possible (see docs/upstream/*.md).

## Hermes

| # | Upstream path | Capability | Reuse | Steward target | Adapter needed | Verification |
|---|---|---|---|---|---|---|
| H1 | `apps/desktop/**` (electron + src) | Desktop shell: windows, HUD, Quick Entry, tray, shortcuts, updater UX | VENDORED | `desktop/hermes/**` (tree kept; later becomes the only desktop app) | Yes — backend-child/probes point at Steward daemon | Electron E2E |
| H2 | `apps/shared/**` | JSON-RPC gateway client + event contract (desktop↔web shared) | LITERAL | `desktop/hermes/apps/shared/**` | No (consumed as-is) | Unit tests ship along |
| H3 | `web/**` | Browser UI (21 pages) | VENDORED | `web-hermes/**` | Yes — `lib/api.ts` transport target → Steward gateway adapter | Browser E2E |
| H4 | `tui_gateway/**` | JSON-RPC gateway server (77 methods) + event stream | VENDORED | `gateway/hermes/**` | Yes — method handlers call Steward daemon (python gRPC client) instead of Hermes agent | Gateway smoke + E2E |
| H5 | branding (productName, icons, titles) | Steward naming | MINIMALLY_MODIFIED | inside vendored trees | package.json + assets swap | build manifests |

## oh-my-pi

| # | Upstream path | Capability | Reuse | Steward target | Adapter needed | Verification |
|---|---|---|---|---|---|---|
| O1 | `packages/tui/**` | Terminal UI library (differential rendering, editor, autocomplete, kill-ring, keys) | VENDORED | `cli-omp/packages/tui/**` | No | upstream unit tests |
| O2 | `packages/coding-agent/**` | The CLI app (`omp`): sessions, commands, composer, tool UX | VENDORED | `cli-omp/packages/coding-agent/**` | Yes — provider/agent-loop boundary | pty E2E |
| O3 | `packages/ai/**` | Provider/auth layer | VENDORED (cut point) | same tree | **Yes — Steward provider facade**: Chat streaming RPC shaped as OMP provider | adapter unit + E2E |
| O4 | `packages/agent/**` | Agent loop | VENDORED (cut point) | same tree | Yes — loop driven by Steward run stream | adapter unit + E2E |
| O5 | `packages/{omptype,utils,natives}/**` etc. | transitive deps of O1–O4 | VENDORED | `cli-omp/packages/**` | No | build green |

## Prime

| # | Upstream path | Capability | Reuse | Steward target | Adapter needed | Verification |
|---|---|---|---|---|---|---|
| P1 | `prime-agent-runtime/src/rlm/**` | Python persistent REPL substrate (NDJSON stdio protocol v3, snapshots, bash, MCP, winjob) | LITERAL | `runtimes/prime-rlm/rlm/**` | Yes — sidecar spawn/shutdown wiring in `kernel/code_runtime` | REPL protocol smoke + E2E |
| P2 | `packages/coding-agent/src/core/rlm-runtime.ts`, `rlm-max-depth.ts` | Run request/subagent registry/depth semantics | LITERAL (as TS kernel-adapter source) | `runtimes/prime-rlm/orchestrator/**` consumed by kernel adapter | Yes — maps to `AgentAction::RlmRun` dispatch | kernel tests + live E2E |
| P3 | `src/modes/daemon/rlm-ledger.ts`, `rlm-subagent-display.ts` | Ledger/display | LITERAL | alongside P2 | Light | unit |

## AutoGen

| # | Upstream path | Capability | Reuse | Steward target | Adapter needed | Verification |
|---|---|---|---|---|---|---|
| A1 | `autogen-studio/frontend/**` (teambuilder + shared components) | Builder UI (graph editor, component editor, validation, test drawer) | VENDORED | `builder-hermes/../builder/autogen-studio-frontend/**` (served as standalone app) | Yes — REST shim → Steward daemon (SaveAgentProfile etc.) | Playwright builder E2E |
| A2 | `autogenstudio/datamodel/**`, `validation/**` | Config schema + validation | VENDORED | inside A1 backend shim | Yes — schema dump ↔ AgentProfile YAML mapping | round-trip tests |
| A3 | `autogen-agentchat/**` team configs | Declarative teams (round-robin/selector/handoff) | VENDORED | schema source for A2 mapping | Yes — maps to kernel `Team*` IR nodes | kernel team tests + E2E |
| A4 | `autogen-core/**` component model | Component config base | VENDORED (only what A1/A2 import) | same tree | No | import graph green |

## Steward-side adapter code (new, minimal)

| Target | Purpose |
|---|---|
| `gateway/steward-bridge/**` | Gateway method handlers → daemon gRPC (python client) |
| `crates/daemon/src/adapter_api/**` (small) | Any missing daemon RPCs the adapters need (extension, not rewrite) |
| `runtimes/prime-rlm/bridge/**` | Sidecar lifecycle + kernel `RlmRun` dispatch |
| builder REST shim | Studio frontend ↔ `SaveAgentProfile/ListAgentProfiles` RPCs |

## Verification matrix

| Phase | Command(s) |
|---|---|
| Hermes Desktop | `cd desktop/hermes/apps/desktop && npm run build` + Playwright `_electron` E2E |
| Hermes Web/Gateway | gateway serve smoke + Playwright browser E2E |
| OMP CLI | `cli-omp` install + build + pty E2E |
| Prime RLM | REPL protocol smoke + `cargo test -p steward-e2e-tests rlm_live` |
| Builder | shim tests + Playwright builder E2E + kernel run test |
| Final | `e2e/all` unified + `cargo test --workspace -- --test-threads=1` |
