# OSS Integration Research — Pinned Upstream Inventory

Four upstream projects are the literal source baseline for Steward's client/harness layers.
Steward's Rust daemon/kernel remains the single authoritative backend; all four surfaces
connect through thin adapters.

| Project | Repo | Pinned SHA | License (code) | Vendored role |
|---|---|---|---|---|
| Hermes Agent | NousResearch/hermes-agent | `693641aa8b4359c602283bdbbc14041e03bc47bc` | MIT (© 2025 Nous Research) | Desktop (Electron) + Web UI + Web Gateway |
| oh-my-pi | can1357/oh-my-pi | `a1b254047d12e143b7c6011536e918c6c35c5906` | MIT (© 2025 Mario Zechner) | CLI/TUI |
| Prime Agent | PrimeIntellect-ai/prime-agent | `844e85545af6858dcb3d6cfe42bbfcf2ca0be4e5` | MIT (© 2025 Mario Zechner) | RLM runtime |
| AutoGen | microsoft/autogen | `027ecf0a379bcc1d09956d46d12d44a3ad9cee14` | MIT via LICENSE-CODE (© Microsoft); CC-BY-4.0 covers docs only | Agent Builder (autogen-studio) |

License audit: all code-bearing trees are MIT → literal vendoring permitted with attribution.
AutoGen docs (`docs/**`, markdown prose) are CC-BY-4.0 — do NOT vendor prose docs; code and
schema files under `LICENSE-CODE` scope only.

Local toolchain verified: Node 24.3 (satisfies hermes `^24.11.0`-adjacent range note), pnpm
10.12, bun 1.4, Python 3.13.

---

## 1. Hermes Agent — Desktop + Web + Gateway

### Source map (pinned tree)

| Path | Files | Lines | Role |
|---|---|---|---|
| `apps/desktop/electron/` | 294 | ~84k | Electron main process: backend lifecycle (spawn/claim/dial/release), window management (popouts, wake indicator, always-on-top HUD), tray, shortcuts, clipboard/WSL bridges, Windows CA/sandbox/profile handling, zoom, updater hooks. One module per concern + colocated `.test.ts`. |
| `apps/desktop/src/` | large | ~447k | React 19 renderer: `store/` (252 files), `app/` (routes incl. floating-hud, command-palette, command-center, artifacts, cron, agents), `api/` (typed gateway calls), `sdk/` (runtime + plugin host), i18n, plugins. |
| `apps/shared/` | 15 | small | **The desktop↔web contract**: `json-rpc-gateway.ts` (typed client, event names, ConnectionState), `websocket-url.ts`, `backend-scope.ts`, billing/cron/skill bridges. |
| `web/` | 171 | ~55k | Browser UI (Vite + React): 21 pages (Chat, Sessions, Files, Models, Mcp, Cron, Skills, Profiles, ProfileBuilder, Logs, Config, System, Pairing…), `lib/api.ts` speaks the same gateway protocol via `@hermes/shared`. |
| `tui_gateway/` | 60 | ~26k | Python gateway server: JSON-RPC over stdio/WS, ~77 registered methods (`prompt.submit`, `session.*` (activate/resume/list/branch/compress/close), `approval.respond`, `complete.slash`, `model.options`, `wake.*`, `cli.exec`, `config.set`, MCP/skills/profiles mgmt), event stream (`message.start/delta/complete`, `tool.start/progress/complete`, `approval.request`, `thinking.delta`…). |

### Gateway event contract (shared, renderer-facing)

Events: `gateway.ready, session.info, session.usage, message.start/delta/interim/complete,
thinking.delta, reasoning.delta, status.update, tool.start/progress/complete,
todo.updated, clarify.request, approval.request, sudo.request, secret.request,
background.complete, error, skin.changed`.

### Backend boundary

Hermes UIs speak JSON-RPC to a Python agent backend (the `tui_gateway` spawns/supervises
the Hermes agent). **Steward adapter strategy:** keep the renderer↔gateway contract intact;
replace/sidecar the gateway's agent-facing calls to Steward daemon (gRPC :50051 / Wire v2 WS).
Two adapter options, decide at implementation:

- **A (preferred):** run `tui_gateway` as a Steward-owned sidecar whose method handlers call
  Steward daemon (Python gRPC client) instead of the Hermes agent.
- **B:** reimplement the ~77-method JSON-RPC surface in a thin Node/TS bridge over Wire v2
  (only if A is blocked by gateway↔Hermes-agent coupling).

### Build/run (upstream baseline)

- Root: `npm ci` (workspace root with `package-lock.json`; `apps/desktop` depends on
  `@hermes/shared` via `file:../shared`).
- Desktop dev: `npm run dev` in `apps/desktop` (vite :5174 + electron); build: `npm run build`;
  Windows installer: `npm run dist:win`.
- Web: `web/` vite app; tests via vitest (`vitest.config.ts`).
- Desktop unit tests: colocated vitest in `apps/desktop/electron/*.test.ts` and `src/*.test.ts`.

### Branding locations

`apps/desktop/package.json` (`productName: Hermes`, name `hermes`), app icons in
`apps/desktop/assets`, window titles inside electron main modules. Rename user-facing only.

---

## 2. oh-my-pi — CLI/TUI

### Source map

| Path | Files | Lines | Role |
|---|---|---|---|
| `packages/tui/` | 151 | ~57k | `@oh-my-pi/pi-tui`: differential-rendering terminal UI library. Modules: `tui.ts` core loop, `terminal.ts`, `editor-component.ts`, `autocomplete.ts`, `fuzzy.ts`, `kill-ring.ts`, `keybindings.ts`, `bracketed-paste.ts`, `mouse.ts`, kitty graphics, tmux integration, desktop-notify. |
| `packages/coding-agent/` | 2879 | ~883k | `@oh-my-pi/pi-coding-agent` — the real CLI (`bin: omp = src/cli.ts`). 61 src dirs: session management, commands/, modes (composer etc.), edit/exec tooling, MCP, config, compress, discovery, collab, autoresearch, goals. |
| `packages/ai/` | — | — | Provider layer: `api-registry.ts`, `providers/`, `auth*` (broker/gateway/retry/storage), dialects. **This is the boundary to cut** → Steward adapter. |
| `packages/agent/` | — | — | Agent loop (`agent-loop.ts`, `agent.ts`, compaction, pause, run-collector, telemetry). Second cut point: loop feeds from Steward streaming instead of OMP's own model calls. |
| `packages/wire/` | — | — | OMP wire protocol — superseded by Steward Wire v2 at the adapter. |

### Backend boundary → Steward adapter

`packages/ai` (provider calls) + `packages/agent` (loop) are the model truth. Adapter plan:
implement a Steward-backed provider/session driver so `omp` runs against the Steward daemon
(Chat streaming RPC) while keeping all TUI/UX code literal. Session storage: OMP's own
session dir → keep for CLI-local UX initially, map/resume to Steward session ids at adapter
(sync layer), since cross-client sessions must resolve to Steward run ids.

### Build/test

Workspace root `package.json` with `packages/*` workspaces; install at root; per-package
`vitest`/test dirs (`packages/tui/test`, `packages/coding-agent/test`). Baseline build green
before any adaptation.

---

## 3. Prime Agent — RLM

### Source map (the real RLM)

| Path | Files | Lines | Role |
|---|---|---|---|
| `prime-agent-runtime/src/rlm/` | 8 | ~4.6k | **Python execution substrate**: `repl.py` (1166 ln) — persistent CPython REPL over newline-delimited JSON stdio (protocol v3, documented in `repl.md`), top-level await, single persistent `__main__` namespace, snapshot/restore (256MB/16MB caps), signal handling, `bash.py` (live-handle kill), `mcp.py`/`mcp_base.py` (MCP in REPL), `harness.py`, `skill.py`, `_winjob.py` (Windows job objects). |
| `packages/coding-agent/src/core/rlm-runtime.ts` | 257 ln | — | TS orchestration: `RlmRunRequest {prompt, kwargs, cellSourceCode}`, subagent registry (running/completed/error), spawn handles (`rlm_child_id`, session dirs), `rlm.list/delete` subagents, model matching (`rlm.find_models`). |
| `packages/coding-agent/src/core/rlm-max-depth.ts` | 13 ln | — | recursion depth cap. |
| `packages/coding-agent/docs/rlm.md`, `docs/rlm-runtime.md` | — | — | protocol/behavior docs (attribution, not vendored prose). |
| `src/modes/daemon/rlm-ledger.ts`, `rlm-subagent-display.ts` | — | — | ledger + UX display layer. |
| `skills/rlm-heartbeat/` | — | — | heartbeat skill. |

### Architecture for Steward

Two-layer literal port:
1. **Runtime substrate (Python):** vendor `prime-agent-runtime/src/rlm/**` as-is →
   `runtimes/prime-rlm/` (Steward already has `runtimes/python` sidecar plumbing).
   Protocol: NDJSON stdio — matches Steward `code_runtime.rs` sidecar contract
   (CreateSession/ExecuteCell/Destroy already defined).
2. **Orchestration (TS):** vendor `rlm-runtime.ts` + `rlm-max-depth.ts` semantics into the
   Steward kernel adapter. `AgentAction::RlmRun` (already in kernel) routes to the Prime
   runtime instead of the current simulated executor. Budget/termination via Prime's own
   limits + Steward `Budget` checks at action dispatch.

Recursive `rlm()` callable inside the REPL spawns child agents → Steward subagent scheduler
(`SpawnMode::Async`), results land back in the persistent namespace as Python values.

### Tests

Upstream has `test/rlm-ledger.test.ts`, `rlm-subagent-display.test.ts`; runtime substrate
is protocol-tested via `repl.md` contract — write Steward integration E2E: live model emits
RlmRun → Prime REPL executes → observation returns to Steward context.

---

## 4. AutoGen — Agent Builder

### Source map

| Path | Files | Lines | Role |
|---|---|---|---|
| `python/packages/autogen-studio/` | 198 | ~38k | Studio: `frontend/` (Gatsby+React; **25 teambuilder TSX files** — `builder/builder.tsx` 602 ln, component-editor, nodes, library, toolbar, validationerrors, testdrawer, store), backend `autogenstudio/` (FastAPI `web/`, `database/`, `datamodel/`, `teammanager/`, `validation/`, `gallery/`, `mcp/`). |
| `python/packages/autogen-agentchat/` | 64 | ~22k | Declarative teams (RoundRobin/Selector/Handoff), component dump/load config model. |
| `python/packages/autogen-core/` | — | — | Component model (config schema, `Component` dump/load) — the schema layer the builder edits. |

### Boundary for Steward

Vendor autogen-studio **frontend** (builder UI) + **datamodel/validation schema** as the
builder surface. Replace the execution backend (`teammanager` running autogen runtime) with
a Steward adapter: builder config → Steward `AgentProfile` (Rust) → kernel execution.
Frontend build: Gatsby (`gatsby-config.ts`), backend: FastAPI — run only the pieces needed;
the builder UI speaks to its own backend via REST — point it at a small Steward adapter API
(FastAPI shim → daemon RPC) rather than rewriting UI components.

Multi-agent composition: agentchat team configs (JSON component model) map onto
Steward `TeamRoundRobin/TeamSelector/TeamHandoff` IR nodes (already in kernel `ir/mod.rs`).

---

## Toolchain & environment notes

- Node 24.3 + pnpm 10.12 + bun 1.4 + Python 3.13.5 present on this Windows machine.
- Hermes desktop requires root `npm ci` then `apps/desktop` scripts; electron 40.
- OMP is bun/TS workspace; tests via package-local runners.
- autogen-studio frontend is Gatsby (older); backend FastAPI; both run offline from repo.
- Steward daemon already serves gRPC :50051 + WebUI :3000 + Wire v2 WS token — the adapter
  target for all four surfaces.

## Key integration risks (to resolve during implementation)

1. **Hermes gateway↔agent coupling:** `tui_gateway` may import Hermes agent internals.
   Mitigation: option A sidecar with method-handler rewrites; inventory import graph first.
2. **OMP model truth:** `packages/ai` provider abstraction is large; the adapter must expose
   a provider-shaped facade over Steward daemon streaming (message deltas, tool calls,
   approvals) — map `message.delta` ↔ Steward `ChatEventKind::Text` etc.
3. **Windows specifics:** Prime `_winjob.py` uses Windows job objects (good — matches
   Steward Windows-first). Hermes electron has `windows-*` modules — keep literal.
4. **autogen-studio frontend is Gatsby:** build isolation needed; embed as separate app
   served by daemon (like current web-ui) rather than merging into Next.js.
5. **Session identity:** CLI-local OMP sessions must map to Steward session ids at the
   adapter for cross-client resume.
