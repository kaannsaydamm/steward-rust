# Parity Plan — cross-harness adoption mapped to Steward crates

Sources analyzed from source (clones under `/tmp/parity-src`, Windows:
`%TEMP%\parity-src`): `NousResearch/hermes-agent`, `can1357/oh-my-pi`,
`PrimeIntellect-ai/prime-agent`, `microsoft/autogen`. Companion research
doc: `docs/HARNESS_UX_RESEARCH.md` (web-sourced, interaction-level).

Brand rule: every ported surface speaks "Steward". Upstream names (hermes,
omp, pi-, prime, autogen, magentic) MUST NOT appear in user-visible strings,
file paths, or symbols. Port behavior and structure, rename identity.

## What each upstream actually contributes (source-verified)

| Upstream | Adopted | Evidence |
|---|---|---|
| hermes-agent | TUI component model (Ink/React), slash registry + fuzzy scorer, model picker dialog (2-stage provider→model, expensive-model confirm), status strip, gateway/sessions overlay, toolset manifest, web ChatPage (1999 ln), desktop composer-dock pattern (`--composer-fill` shared var), status-dot tones, queued-messages UI | `ui-tui/src/app/slash/{registry,fuzzyScore}.ts`, `web/src/components/{SlashPopover,ModelPickerDialog,SidebarStatusStrip}.tsx`, `apps/desktop/src/components/chat/composer-dock.ts`, `tools/toolsets.py` |
| oh-my-pi | 99 tool implementations (read/write/edit/ast-grep/bash/checkpoint/computer/run-code/sqlite/pdf...), slash command families (registry/modes/session/lifecycle), TUI editor stack (autocomplete, kill-ring, bracketed paste), lazy `xd://` tool discovery | `packages/coding-agent/src/{tools,slash-commands}/*`, `packages/tui/src/{autocomplete,fuzzy,keybindings}.ts` |
| prime-agent | RLM runtime (257 ln core), refinement engine (1015 ln), persistent Python REPL tool (`ipython.ts`), session daemon/worker split, footer-data-provider | `packages/coding-agent/src/core/{rlm-runtime,refinement,footer-data-provider}.ts` |
| autogen | Team orchestration patterns: RoundRobin/Selector/Swarm/MagenticOne group chats, graph flows; Studio frontend structure | `autogen_agentchat/teams/_group_chat/*` |

Steward already has crates for most backend concepts (`kernel` TurnEngine +
IR scheduler + HITL; `harness` agents/scheduler/mailbox; `coding` reader/edit;
RLM sidecar in `kernel/code_runtime`). The parity gap is **surface**: CLI/TUI,
web chat interactions, and the desktop shell.

## Phase P0 — Web chat reaches harness-grade (this ship)

Owner-confirmed pain: `/` does nothing; `Aa·M ⟷·M New` cryptic. Worker
`harness-ui` is implementing now:

1. CommandPalette (Hermes `SlashPopover` + `fuzzyScore` port): trigger `/`
   at input start; fuzzy tiers exact(0)/prefix(1)/substring(2), description
   words +3; ↑↓ navigate, Enter/Tab execute, Esc close; rows carry
   description + keybinding hint; availability rules (`/compact` needs a
   session — hidden with visible rule, anti-pattern #7).
2. Toolbar: kill `Aa·M ⟷·M New` → labeled icon buttons with tooltips +
   aria-labels (Type/MoveHorizontal/Plus/Compact), same cycling + storage.
3. Model picker: 2-stage (provider→profile) like Hermes `ModelPickerDialog`,
   check-marked current, switch applies next turn, session preserved
   (anti-pattern #8). Data: `listProviderProfiles` + activate RPC.
4. Status bar (Hermes `SidebarStatusStrip` + Claude statusline fields):
   session id │ model │ tool count │ daemon dot; `—` for unknown usage
   (anti-pattern #4). Auto-grow composer (min 1, max 8 rows, then scroll).
5. i18n: every new string in `en.ts` + `tr.ts`.

## Phase P1 — Interaction depth (next)

6. Message queue while busy (Claude/Codex/Cursor converge; anti-pattern #3):
   typed Enter queues above composer, `↑` recalls; flush after turn. Needs
   daemon turn-end signal — exists (`ChatEventKind` stream end).
7. `@` file mentions → context pills above composer (Cursor pattern); pills
   sent as structured context; requires daemon file-list RPC (add
   `ListWorkspaceFiles` to `steward.proto`, backed by `coding::reader`).
8. Input history (`↑`/`↓` at edges), `Esc` interrupt, `Esc Esc` clear draft
   (≤800ms window), `Ctrl+S` stash, `Ctrl+J` newline fallback, `?` help
   overlay on empty input.
9. ToolStepGroup upgrade (anti-pattern #1): spinner while `TOOL_START` w/o
   `TOOL_RESULT`, per-call latency right-aligned, group total duration,
   expansion memory per block id, right-rail grouped by turn.
10. Keyboard map: `Alt+1..9` tab jump (page.tsx), `Ctrl+K` palette when
    composer focused+empty, `Ctrl+O` raw transcript view.

## Phase P2 — Desktop shell goes Hermes-native (Tauri)

Current `desktop/src-tauri` is supervisor+URL window. Port hermes desktop
concepts with Steward branding:

11. Composer-dock layout: shared `--composer-fill` surface + status/queue
    stack + floating pill buttons (port `composer-dock.ts` classes into
    web-ui globals.css as `.composer-*`; both web + desktop consume).
12. Pane shell: left sessions tree, center chat, right inspector (model/
    tools/artifacts); panes persisted per workspace (`pane-lifecycle`
    pattern, localStorage keys `steward.pane.*`).
13. Session quick-switcher popup (Hermes `activeSessionSwitcher`): type-to-
    filter over `listChatSessions`, `Ctrl+X` open.
14. Onboarding: first-run overlay → setup wizard reuse (CLI `setup --quick`
    logic surfaced as dialog); boot-failure overlay with retry.
15. Tray: daemon state dot tones (good/warn/bad → primary/amber/error),
    "Open Steward", "Stop daemon (spawned only)".

## Phase P3 — CLI/TUI (cargo steward-cli tui)

16. Port omp TUI editor stack semantics to ratatui TUI: autocomplete menu,
    kill-ring, bracketed paste; slash registry mirroring web palette
    (single source: `harness::slash` registry consumed by both web (via
    RPC `ListSlashCommands`) and CLI).
17. `!` shell mode + `@` file mention in TUI composer (omp/bash patterns;
    tool policy effect checks enforced — `tools::policy`, no bypass).
18. Status footer via prime-agent `footer-data-provider` split: daemon-fed
    struct → render layer.

## Phase P4 — Agent teams (autogen parity in kernel)

19. `kernel::ir` gains team topologies beyond fan-out/fan-in:
    RoundRobin (sequential rotation), Selector (model-chosen next speaker),
    Swarm (handoff), GraphFlow (existing IR graph covers). Implement as IR
    node kinds + scheduler policies with termination (budget + max-round).
20. Magentic-style orchestrator: ledger (facts/plan) maintained in harness
    memory between supersteps; reset on stall counter. Tests with
    ScriptedModel: rotation order, selector choice, handoff termination,
    ledger reset on stall.

## Phase P5 — Tools + RLM depth

21. Tool expansion from omp's 99: prioritize by Steward gaps — sqlite-reader
    (have: coding/reader sqlite-meta → add row reads), pdf text extract,
    checkpoint tool (kernel checkpoints exist; expose as tool), think/todo
    tools (harness scratchpad). Each lands with tests, no stubs.
22. RLM surface: prime-agent's REPL-as-tool UX — chat `/rlm` opens a
    persistent session panel; tool `run_rlm(code, session)` backed by
    existing `kernel/code_runtime` sidecar; results as observations with
    provenance (context::Observations). Subagent call `rlm(...)` inside REPL
    maps to harness scheduler spawn — wired, tested.

## Verification contract (applies to every item)

- Each numbered item: implementation + behavior tests (web: tsc+build +
  browser smoke via browser device; Rust: `cargo test -p <crate>`; e2e where
  RPC contract changes: `tests/`).
- Brand sweep before merge: `grep -ri "hermes\|oh-my-pi\|\bomp\b\|prime-agent\|autogen\|magentic" web-ui/src desktop crates/*/src --include="*"` → only
  allowed in this doc + docs/HARNESS_UX_RESEARCH.md.
- No stub markers: `grep -rn "TODO\|FIXME\|unimplemented!\|todo!" changed-paths`
  clean at each commit.

## Status

- [x] Research doc (web + source analysis)
- [ ] P0 items 1-5 (worker `harness-ui` in flight — director verifies)
- [ ] P1-P5
