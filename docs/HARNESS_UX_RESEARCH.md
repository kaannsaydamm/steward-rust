# Harness UX Research — input-driven interaction patterns for the Steward Web UI

Research basis for the web-ui interface overhaul. Sources: official docs, GitHub repos/READMEs, and reference pages read 2026-09-07; URLs inline per section. Component references use the existing files (`web-ui/src/components/ChatTab.tsx`, `web-ui/src/components/ToolStepGroup.tsx`, `web-ui/src/components/Sidebar.tsx`, `web-ui/src/app/page.tsx`).

---

## 1. Per-harness findings

### 1.1 Claude Code (Anthropic CLI)

Sources: <https://code.claude.com/docs/en/interactive-mode>, <https://code.claude.com/docs/en/commands>, <https://code.claude.com/docs/en/statusline>, <https://code.claude.com/docs/en/skills>

**Slash command menu.** `/` at input start opens a filterable menu of built-in commands, bundled skills, user skills, plugin commands, and MCP prompts. Typing filters; a few hidden commands run only when their full name is typed. A command is only recognized at the start of the message; trailing text becomes arguments. Commands typed while Claude is responding are queued and run after the turn — except immediate ones (`/status`, `/tasks`, `/usage`, and in fullscreen `/theme`, `/help`) which run right away. Skills can be chained: up to six `/skill-a /skill-b args` invocations in one input. Source precedence (enterprise > personal > project > bundled) is resolved so the menu shows one canonical entry per name.

**Keyboard map (verified subset).**

| Key | Behavior |
|---|---|
| `Tab` | Accept autocomplete suggestion; also opens a comment field on permission prompts |
| `Esc` | Interrupt Claude mid-turn (work done so far is kept); close dialogs; declines permission |
| `Esc Esc` | With text: clear draft (saved to history). Empty: open rewind/checkpoint menu |
| `Shift+Tab` | Cycle permission modes: default → acceptEdits → plan → (bypass → auto) |
| `Up/Down` | Move cursor within wrapped/multiline input first; then navigate history; `Up` from first row takes back queued messages |
| `Ctrl+R` | Reverse history search (fullscreen: dialog with scope cycling session/project/all) |
| `Ctrl+O` | Transcript viewer: detailed tool usage with timestamps/model per message; expands collapsed MCP calls |
| `Ctrl+T` | Toggle task checklist (up to five tasks shown in status area) |
| `Ctrl+B` | Background running Bash/agents |
| `Ctrl+G` / `Ctrl+X Ctrl+E` | Edit prompt in `$EDITOR` |
| `/` `!` `@` `:` `?` | Prefix triggers: commands, shell mode, file mention (also suggests other live sessions), emoji shortcodes, shortcut-help panel on empty input |

**Multiline.** `Shift+Enter` (native in Windows Terminal/iTerm2/Kitty/WezTerm/Ghostty/Warp), `\`+`Enter`, `Option+Enter`, `Ctrl+J` as universal fallback, and paste mode. readline editing (`Ctrl+A/E/K/U/W/Y`, `Alt+B/F/D`).

**Queued messages.** Typing while Claude works queues instead of interrupting; queue is listed above the input box; delivered per rule (messages pass to Claude after in-flight tool calls; commands run at turn end). `Up` from the first row recalls and removes queued items.

**Prompt suggestions.** Grayed ghost suggestion for the next prompt; `Tab`/`→` to accept, typing dismisses. Suppressed in plan mode, cold cache, near usage limits.

**Status line.** Customizable bottom bar fed by JSON on stdin: `model.display_name`, `workspace.current_dir`, `context_window.used_percentage`, `cost.total_cost_usd`, `cost.total_duration_ms`, `cost.total_lines_added/removed`, rate-limit percentages, `output_style.name`, `vim.mode`, PR badge fields (`pr.number`, `pr.review_state` → green approved / yellow pending / red changes requested / gray draft). Event-driven updates (new assistant message, compaction, mode change) plus optional `refreshInterval`; 300 ms debounce; multi-line output and ANSI colors supported.

**Tool rendering.** Collapsed by default; `Ctrl+O` transcript view expands. `/diff` shows a live file list with +/− line counts; in fullscreen a side diff panel updates as edits happen and stays out of test/generated files; `Ctrl+X B` cycles the diff base (session changes → uncommitted → since branch split).

**Plan mode.** `/plan` switches modes; mode shown in the mode indicator; on Opus/Sonnet 4.8+ Claude plans without a written checklist; the task list (`Ctrl+T`) shows pending/in-progress/complete indicators and survives compaction.

### 1.2 OpenAI Codex CLI

Sources: <https://developers.openai.com/codex/cli/slash-commands> (served as `learn.chatgpt.com/docs/developer-commands?surface=cli`), <https://learn.chatgpt.com/docs/agent-approvals-security.md>, <https://learn.chatgpt.com/docs/cli-customization.md>, <https://github.com/openai/codex>

**Slash popup.** Type `/` to open the slash popup, keep typing to filter, `Enter` to run. While a turn runs, typing a command + `Tab` queues it for the next turn (errors/menus appear when it executes); completion still works before queueing. Full command set includes `/model` (with reasoning effort), `/permissions` (approval preset picker: e.g. `Auto`, `Read Only`, plus custom permission profiles), `/plan`, `/goal` (persistent objective with pause/resume/edit/clear, progress row above the composer), `/compact`, `/copy` (also `Ctrl+O`), `/diff`, `/status`, `/usage`, `/mention` (attach file), `/fork`, `/resume`, `/rename`, `/new`, `/archive`, `/delete`, `/skills`, `/import`, `/statusline`, `/title`, `/theme`, `/keymap` (interactive TUI re-binding persisted to `tui.keymap` in `config.toml`).

**Approval modes UI.** `/permissions` opens a preset picker; presets map to sandbox × approval combos (e.g. `Auto` = `--sandbox workspace-write --ask-for-approval on-request`). Destructive app/MCP tool calls always require approval. `/status` prints active model, approval policy, writable roots, token usage. `approvals_reviewer = "auto_review"` routes eligible approval prompts through a reviewer agent instead of the user.

**Diff display.** `/diff` shows the Git diff including untracked files, rendered with syntax highlighting for diffs in the TUI.

**Interactive shortcuts (documented).** `Up`/`Down` restore draft history; `Ctrl+R` searches prompt history (`Enter` uses match, `Esc` cancels); `Ctrl+O` copies the latest completed response; `!` prefix runs a local shell command under current approval/sandbox settings; `Tab` while working queues a follow-up (prompt, slash command, or shell command); `Enter` while working injects new instructions into the current turn; `Esc Esc` with empty composer edits the previous user message and forks the chat from that point; `Ctrl+C` exits.

**Status line.** `/statusline` opens an interactive picker to toggle and reorder footer items: model, model+reasoning, context stats, rate limits, git branch, token counters, session id, cwd/project root, version — persisted to `tui.status_line`. `/title` does the same for the terminal tab title.

### 1.3 Gemini CLI

Sources: <https://raw.githubusercontent.com/google-gemini/gemini-cli/main/docs/reference/keyboard-shortcuts.md>, <https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/plan-mode.md>, <https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/custom-commands.md>, <https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/cli-reference.md>

**Slash commands.** `/help`, `/quit`, `/settings`, `/plan [goal]`, `/vim`, `/memory reload`, `/mcp reload`, `/commands list|reload` (custom slash commands from `~/.gemini/commands/*.toml` and `.gemini/commands/*.toml`; project overrides user; subdirectories become namespaced `/git:commit`). Custom command prompts support `{{args}}` raw injection, `!{shell command}` output injection with confirmation dialog and shell-escaped args, and `@{path}` file-content injection (multimodal for images/PDF).

**`!` shell mode.** `!` on empty prompt enters/exits shell mode; commands run in the session working directory. (Distinct from Claude Code's shell mode, which also feeds output into context.)

**Approval modes.** `Shift+Tab` cycles default → auto_edit → plan (plan skipped while busy). Plan mode is a read-only policy-engine state with allowed-tool restrictions; plans are written as Markdown files (default `~/.gemini/tmp/<project>/<session-id>/plans/`); `Ctrl+X` opens the plan in an external editor for collaborative editing; approval dialog offers "Yes, automatically accept edits" / "Yes, manually accept edits" / cancel (`Esc`).

**Keyboard reference (relevant rows).**

| Command | Keys |
|---|---|
| `input.submit` | `Enter` |
| `input.newline` | `Ctrl+Enter`, `Cmd/Win+Enter`, `Alt+Enter`, `Shift+Enter`, `Ctrl+J` |
| `input.queueMessage` | `Tab` (queues the prompt while a task runs) |
| `input.openExternalEditor` | `Ctrl+G` |
| `history.search.start` | `Ctrl+R` |
| `suggest.accept` | `Tab` / `Enter`; `Up`/`Down` (`Ctrl+P/N`) navigate completions |
| `app.cycleApprovalMode` | `Shift+Tab` |
| `app.showFullTodos` | `Ctrl+T` |
| `app.clearScreen` | `Ctrl+L` |
| `edit.*` | readline set (`Ctrl+A/E/K/U/W`, `Ctrl+Backspace` word delete, undo `Ctrl+Z`) |

**Other notable UX.** `?` on empty prompt toggles a shortcuts panel above the input (auto-hides while streaming); `Esc` twice clears input or rewinds previous interactions; `Up`/`Down` at input edges navigate history; number keys `1-9` jump directly to numbered radio options in dialogs; `Tab Tab` toggles minimal/full UI details; paste placeholders (`[Pasted Text: X lines]`) expand with `Ctrl+O` or double-click. Keybindings are user-configurable via `~/.gemini/keybindings.json` (VS Code schema; `-command` unbinds).

### 1.4 Hermes Agent (NousResearch) — exact repo found

Source: <https://github.com/NousResearch/hermes-agent> (242k+ stars, MIT, Python), docs at <https://hermes-agent.nousresearch.com/docs/user-guide/cli> and <https://hermes-agent.nousresearch.com/docs/user-guide/tui>. Not a substitute: this is the repo the task names.

**Interface layout.** Banner (model, terminal backend, working dir, tools, skills) + conversation stream + fixed input prompt + persistent **status bar above the input**:

```
⚕ claude-sonnet-4-20250514 │ 12.4K/200K │ [██████░░░░] 6% │ $0.06 │ 15m
```

Elements: model name (truncated at 26 chars), token count used/max, color-coded context bar (green <50%, yellow 50–80%, orange 80–95%, red ≥95%), session cost, 🗜️ auto-compression count, ▶ active background-task count, elapsed duration, gold session-title badge pinned far right, ⚠ YOLO badge whenever auto-approve is on. Bar adapts to terminal width (full ≥76 cols, compact 52–75, minimal <52).

**Keybindings (documented).** `Enter` send; `Alt+Enter`/`Ctrl+J`/`Shift+Enter` newline; `Ctrl+G` / `Ctrl+X Ctrl+E` open buffer in `$EDITOR`; `Ctrl+S` stash prompt (stack with browse panel; 📌 N badge in status bar); `Ctrl+C` interrupt (double-press 2 s force-exit); `Tab` accept ghost-text auto-suggestion or slash autocomplete; `!` shell mode (zero cost, nothing enters conversation, approvals still apply, shows `! exited <code>`); multiline paste collapses to `[pasted: 47 lines, 1,842 chars — press Enter to send]`.

**Slash autocomplete + TUI overlays.** `/` opens an autocomplete dropdown (floating panel with descriptions in the TUI). Commands are case-insensitive; installed skills become slash commands automatically. TUI renders model picker, session picker, approval, and clarification prompts as modal panels; `/help` is an overlay with categorized commands, arrow-key navigable; `/sessions` is a live session switcher (↑/↓ navigate, `Enter` switch, `Ctrl+D` close, `Ctrl+N` new, `Ctrl+R` refresh, `Esc` close); `/agents` shows a live subagent tree with kill/pause controls and per-branch cost/token/file rollups; `/context` renders a glyph-block grid of context usage by category; `/details` toggles verbose tool-call details globally or per-section (hidden/collapsed/expanded accordion states configurable per section: thinking, tools, activity).

### 1.5 oh-my-pi (OMP harness)

Source: <https://github.com/can1357/oh-my-pi> (MIT, TypeScript + ~80k LoC Rust), docs at <https://omp.sh/docs/tools>, <https://github.com/can1357/oh-my-pi/blob/main/docs/agent-hub.md>. Exact repo found — not a substitute.

**Lazy tool discovery.** 31 built-in tools always in the same namespace as `read`/`bash`; rarely-used tools are "discoverable" behind `xd://` devices: `read xd://` lists them; `write xd://<tool>` executes one. Set-gated tools (github, security_scan, image gen, memory) are off by default and activated by config. Contrast: nothing is hidden behind a settings screen — the agent surface enumerates what exists on demand.

**Tool-call rendering.** Every tool call renders as a card; edits preview before they land; ambiguous requests route through the `ask` tool — a structured option picker the agent calls mid-turn (multi-select supported), surfaced identically over ACP to editors. `ast_edit` returns a `(proposed)` card with replacement count; the TUI turns a follow-up write into an **Accept** card that commits atomically. Time-traveling stream rules render as amber `⚠ Injecting rule` cards mid-stream. Advisor notes render as `Advisor N note (concern)` cards.

**Agent Hub (`Alt+A`, also `Ctrl+S`).** Live roster of subagents: status (running/idle/parked/aborted), identity, parent, unread-message count, model role, resolved model, age since last activity, assigned task, cost, elapsed time, request/tool-call counts, tokens. Header aggregates totals. `t` toggles flat vs parent/child tree; `Tab` opens inspector (current tool + args, last intent, retry state, context-window use, lineage, output/patch paths); `Enter` focuses a subagent's live transcript and lets you type steering messages; `r` revives parked agents; `x` kills; `Esc` steps back. Missing data renders as `usage —`, never an estimate.

**Model routing.** Nine roles (default, smol, slow, plan, commit, vision, task, advisor, tiny) route by intent; `Ctrl+P` cycles models for the active role; `/model` swaps mid-session; per-role fallback chains, path-scoped model sets, round-robin credentials.

**Other surfaces.** `/collab` share link/QR with read-only `view` variant; hashline content-hash edit anchors; internal URL schemes (`pr://`, `agent://`, `history://`) unify FS-shaped tools; persistent Python/Bun eval kernels that call back into agent tools.

### 1.6 VS Code Command Palette

Sources: <https://code.visualstudio.com/docs/editing/getting-started/userinterface>, <https://code.visualstudio.com/docs/configure/keybindings>, <https://code.visualstudio.com/docs/reference/default-keybindings>

- `Ctrl+Shift+P` (or F1) = Show All Commands; `Ctrl+P` = Quick Open (files); both share one quick-input widget with mode prefixes.
- Type `?` in the input to list all prefix modes (documented tip in the userinterface page).
- Fuzzy matching across the command label; results are ordered MRU-first for recently used commands (the `Ctrl+Tab` editor-history command is explicitly `quickOpenPreviousRecentlyUsedEditorInGroup` in the default-keybindings reference, evidencing the MRU model).
- Every row shows: command title, (optionally) the category it belongs to, and the current **keybinding hint right-aligned** in the row; unbound commands show nothing there.
- Keyboard: `Up`/`Down` navigate, `Enter` accept, `Esc` dismiss; `Ctrl+Tab` cycles recently used editors.
- The palette is draggable/repositionable ("Quick Input Positions" in Customize Layout).
- Keybinding rules: evaluated bottom-to-top, first match wins; chords supported (`Ctrl+K Ctrl+S`); user overrides append after defaults so they win. The Keyboard Shortcuts editor (`Ctrl+K Ctrl+S`) lists all commands with/without bindings and can filter `@source:user`.

### 1.7 Cursor / Windsurf chat input

Sources: <https://cursor.com/docs/agent/prompting.md>, <https://cursor.com/docs/agent/overview.md>, <https://cursor.com/docs/agent/plan-mode.md>, <https://cursor.com/docs/cli/reference/slash-commands.md>; Windsurf is now Cognition Devin Desktop — Cascade docs: <https://docs.devin.ai/desktop/cascade/memories.md>

**Cursor @ mentions** (documented in `agent/prompting.md`): `@` in the chat input opens matching suggestions; categories include Files & Folders (`@auth.ts`, `@src/components/` with `/` to drill deeper), `@Terminals` (terminal output), `@Chats` (previous conversations), `@Commit (Diff of Working State)` / `@Branch (Diff with Main)`, `@Browser`. Docs explicitly advise skipping @ mentions when unsure — agent search covers it.

**Modes and model picker.** Mode dropdown + `Shift+Tab` rotates to Plan Mode (also auto-suggested by task keywords). Model picker at the top of the chat input; `Cmd+/` cycles models; change applies going forward in the conversation. `/skill` menu: `Enter` attaches a skill to one message; `Alt+Enter` (Option+Enter on Mac) or "Use as Mode" keeps the skill active as a Custom Mode on every turn. Slash set in Cursor CLI: `/model`, `/plan`, `/ask`, `/debug`, `/goal`, `/resume`, `/fork`, `/summarize`, `/rewind`, `/shell`, `/vim`, `/mcp`, `/sandbox`, etc.

**Queued vs steering.** While Agent works: `Enter` queues the message (listed in order below the active task, drag to reorder); `Cmd+Enter` sends immediately and appends to the latest user message, steering at the next tool-call boundary; "Send now" button / pressing Enter twice does the same; `Tab` queues for after the turn. In Cursor CLI, first `Enter` steers at a safe boundary, second interrupts.

**Context usage ring.** A context ring next to the prompt input shows fill level; clicking opens a breakdown tray: system prompt / tools / rules / skills / MCP / subagents / summarized conversation / conversation, with hover-to-highlight linking ring segments to rows.

**Checkpoints.** Automatic snapshots before significant changes; click any checkpoint in the chat timeline to preview and restore (files only, not messages); restore button on each prior request.

**Windsurf/Cascade (post-Cognition rename, documented in Devin Desktop docs).** Skills invoked via `@mention` or model-decided; Workflows invoked only manually via `/[workflow-name]`; Rules have four activation modes (`always_on`, `model_decision` — description always, full content on demand, `glob`, `manual` — activated by typing `@rule-name` in the input box). Note: exact Windsurf chat-input key conventions were not verifiable from current (Devin-branded) docs; treat Cascade-specific composer details as unverified and prefer the Cursor/Claude Code conventions above.

---

## 2. Adopt for Steward

Prioritized spec. Components touched are the actual files in `web-ui/`. "P0" = implement first.

### A. Slash command palette — P0

Touch: `ChatTab.tsx` (new overlay + input handling); command registry can start local to ChatTab.

- **Trigger:** `/` as the first character of the input opens the palette; subsequent typing filters; deleting back past `/` closes it. Also `Ctrl+K`-style global: `Ctrl+K` with empty input opens the palette in "command mode".
- **Rows:** `command` (mono, left), `description` (muted, right or second line), source badge (`builtin` / `skill`), and keybinding hint if the command has one — the VS Code pattern.
- **Filtering:** case-insensitive substring + simple subsequence fuzzy (Gemini/VS Code style). Empty query shows all, ordered: session-relevant first (e.g. `/compact` only when a session is open — mirror Claude Code's per-availability menu), then alphabetical.
- **Navigation:** `↑`/`↓` move, `Enter` accept, `Tab` accept-with-args-focus (cursor lands after the command word), `Esc` closes and keeps the input text. Mouse hover highlights; click accepts.
- **Arg-bearing commands** (`/model`, `/plan`, `/title`) keep the palette open until a space is typed, then close into free-text entry — Codex's "type `/plan <inline>`" behavior.
- **Queue semantics (P1):** commands typed while `busy` are queued (Codex: `Tab` queues; Claude Code queues automatically). Until streaming cancellation exists, run immediately-available commands (model switch, session info) even while busy and queue turn-boundary ones.
- **Initial set:** `/new`, `/compact` (exists: `compactSession()`), `/model`, `/tools`, `/sessions`, `/clear`, `/help`, `/plan` (stub until plan mode ships), `/status`.

### B. @ mentions / context pills — P0

Touch: `ChatTab.tsx` input handling; backend already accepts `workingDirectory` in the chat call (see `submit()`); mention resolution needs a daemon file/workspace listing RPC.

- **Trigger:** `@` anywhere in the input opens a file suggestion popup filtered by the text after `@`; `/` inside a mention drills into folders (Cursor pattern).
- **Rows:** icon (file/folder/session), relative path, and (P1) mtime. Cursor also offers non-file categories — Steward's equivalents: `@Sessions` (existing chat sessions), `@Artifacts` (ArtifactsTab items), `@Knowledge` (KG nodes). File-only is the P0 cut.
- **Acceptance:** `Enter`/`Tab`/click inserts a mention token rendered as a **context pill** (rounded chip, mono filename, `×` to remove) between the textarea top and the input text — the Cursor/Claude Code "chip above the composer" pattern, not inline mangling.
- **Sending:** pills are attached to the next prompt (sent as structured context in the gRPC request, not as `@path` text); they expire after one message (Cursor skill behavior) unless pinned.

### C. Tool-call strip improvements — P0 (already partially built)

Touch: `ToolStepGroup.tsx`, `ChatTab.tsx`.

Existing state: consecutive tool messages collapse into one grouped strip with pills and an expandable per-call list. Gaps vs researched harnesses:

1. **Live status per call:** each row shows a spinner while running (Claude Code/Hermes stream tool output), a checkmark on success, and a red marker on error. Today `content` only arrives after the fact; add an in-progress marker when `TOOL_START` fired but no `TOOL_RESULT` yet.
2. **Summary-first line:** first line of content is already shown; add per-call latency (`1.2s`) right-aligned, muted — Hermes and omp show per-call cost/duration in the roster; here per-call latency is the cheap analog.
3. **Group header counts:** header shows `N tool steps` (exists) + unique tool names (exists) + total duration (new).
4. **Default collapse, remember expansion** per message-block id (Hermes `/details hidden|collapsed|expanded` semantics: hidden = header only, collapsed = one line per call, expanded = full output).
5. **Right-rail mirror** (the tool list aside in ChatTab) should group by turn rather than a flat list, and jump to the transcript block on click.

### D. Status bar — P1

Touch: `ChatTab.tsx` (new bar between transcript and composer); `page.tsx` daemon status feeding.

Single row above the composer, mono 10–11px, contents modeled on Hermes's bar and Claude Code's statusline fields:

```
⌥ sonnet-4.6 │ 38.2K/200K [██░░░░░░░░] 19% │ $0.14 │ 4m12s │ ⚙ 12 tool calls
```

- Model display name (currently only in the right rail — keep both; the bar is the primary).
- Context meter: used/max tokens with a color-coded bar (green <50 / yellow 50–80 / orange 80–95 / red ≥95 — Hermes thresholds). Requires the daemon to report token usage per turn; until then show `—` rather than estimating (omp rule: missing data renders as `usage —`).
- Session cost if the provider returns usage; elapsed turn time while busy, frozen after completion (Hermes `⏱/⏲` behavior).
- Busy indicator with a **Stop** affordance on the right once streaming cancellation exists.
- Adaptive: on narrow widths drop to model + time (Hermes width tiers).

### E. Input area conventions — P0

Touch: `ChatTab.tsx` composer (`<textarea>` + form).

- **Enter submits, Shift+Enter newlines** — already implemented (`onKeyDown` Enter/!shiftKey); keep. Add `Ctrl+J` as terminal-portable newline fallback (Gemini's universal fallback; also covers Windows Terminal edge cases).
- **Auto-grow:** replace fixed `rows={2}` with auto-height growth (min 1 line, max ~8 lines, then internal scroll). Claude Code/Gemini/Cursor all grow; a fixed 2-row box is the "primitive" signal.
- **Interrupt:** while `busy`, Enter must not be silently swallowed: pressing Enter with text should **queue** the message (Claude Code/Codex/Cursor converge on this), shown in a queue list above the composer; the send button becomes **Stop** once cancellation exists; until then the button stays disabled and queued messages flush after the turn.
- **History:** `↑`/`↓` at the first/last visual row recall prior prompts (Claude Code rule: cursor moves first, history only at edges). Store per session, dedupe consecutive duplicates.
- **Draft stash:** `Ctrl+S` parks the draft and restores it on the next press (Hermes stack semantics; one-slot is enough for P0).
- **External editor / paste:** P2 — paste of large multiline content collapses to `[pasted: N lines]` chip (Hermes/Gemini pattern); `Ctrl+G` handoff to external editor is low priority for a web UI.
- **Placeholder coaching:** default placeholder enumerates the trigger grammar: `Message, / commands, @ files` (VS Code's `?`-hint idea compressed).

### F. Model picker placement — P1

Touch: `ChatTab.tsx` (new inline picker); model list already available via `listProviderProfiles`.

- A compact **model chip button inside the composer footer row** (left side of the status bar / bottom-left of the input), not only the right rail: Cursor puts the picker at the top of the chat input, Codex via `/model` popup, ChatGPT desktop binds `Ctrl+Shift+M`. For a web console, bottom-left chip next to the send area is least disruptive.
- Opens a popup grouped by provider with cost/context hints per row (Hermes TUI model picker is grouped by provider with cost hints); keyboard `↑`/`↓` + `Enter`; current model check-marked. `Ctrl+/` cycles to the next model without opening the popup (Cursor `Cmd+/`).
- Switching mid-session applies to the next turn and confirms in-transcript (Codex behavior).

### G. Keyboard shortcut map — P1

Touch: `ChatTab.tsx` + `page.tsx` (global layer); `?`-on-empty pattern (Claude Code + Gemini both do this).

| Binding | Action | Layer |
|---|---|---|
| `Enter` / `Shift+Enter` / `Ctrl+J` | submit / newline / newline | ChatTab |
| `/` (input start) or `Ctrl+K` | command palette | ChatTab |
| `@` | mention popup | ChatTab |
| `↑`/`↓` at edges | history | ChatTab |
| `Esc` | interrupt turn / close popup | ChatTab |
| `Esc Esc` | clear draft (keep in history) | ChatTab |
| `Ctrl+S` | stash/restore draft | ChatTab |
| `Ctrl+/` | cycle model | ChatTab |
| `Ctrl+T` | toggle tool-activity panel | ChatTab |
| `Alt+1..9` | jump to Sidebar tab | page.tsx (Sidebar.tsx exposes `TabId` order) |
| `?` on empty input | shortcut help overlay | ChatTab |
| `Ctrl+O` | full transcript view (raw, ungrouped) | ChatTab — analog of Claude Code transcript viewer |

Rules: two-press `Esc` needs a ≤800 ms window (Claude Code); chord-like `Ctrl+K` must not fight browser shortcuts when focus is inside a text field — only bind it when the composer is focused and empty.

### H. Session & mode surfaces — P2

- **Permission/mode selector:** a small `Shift+Tab`-cycled mode chip (Chat / Plan / Auto-approve) above the composer once plan mode and tool approval exist; mode must be visible while the agent works (Claude Code mode indicator + Hermes YOLO badge: never let an auto-approving state be invisible).
- **Session switcher:** promote the left session list into a `Ctrl+X`-style quick-switcher popup with type-to-filter (Hermes live session switcher) rather than a hover-to-delete list; keep the sidebar for history browsing.
- **Agent/queue panel:** when subagents exist, an `Alt+A` Agent-Hub-style roster (status, model, elapsed, tokens) replaces the current flat tool list in the right rail.

---

## 3. Anti-patterns to avoid (from research)

1. **Tool output as prose interleaving.** Every harness (Claude Code, Hermes, omp, Codex) collapses tool activity into compact strips/cards with opt-in expansion. Dumping raw tool results between messages is the defining "primitive" smell — Steward's `ToolStepGroup` is the right direction; extend it, don't abandon it.
2. **Invisible state.** Hermes renders a permanent ⚠ YOLO badge; Claude Code shows the mode indicator; Codex `/status` prints approval policy. Any auto-approval/plan/mode state that isn't constantly visible is an anti-pattern.
3. **Blocking the composer while busy.** Claude Code queues, Codex distinguishes queue (`Tab`) vs inject (`Enter`), Cursor queues vs steers. Disabling the textarea during a turn (Steward's current `busy` state) is the worst option: it blocks steering and forces the user to wait or lose their draft.
4. **Estimating missing data.** omp renders missing usage as `usage —`. Never display invented token/cost numbers in the status bar; show a dash until the daemon reports real values.
5. **Command menus without metadata.** VS Code rows carry keybinding hints and categories; Claude Code/Hermes rows carry descriptions and source badges. A bare list of command names (no description, no availability logic) fails discovery.
6. **Overloading Enter with no escape hatch.** All researched harnesses provide an explicit newline binding that works in every terminal/browser (`Ctrl+J` for Gemini/Claude Code). Relying only on `Shift+Enter` breaks in IME/mobile-keyboard contexts; relying on Enter-with-modifier conventions that the platform swallows (Hermes notes Windows Terminal eats `Alt+Enter`) is documented as a known trap.
7. **Hiding commands behind context the user can't see.** Claude Code deliberately hides some commands from the menu until fully typed; that's fine — but availability-dependent commands (`/compact` without a session) must either be hidden with a visible rule or shown-disabled with a reason. Showing them and failing at runtime is the anti-pattern.
8. **Model switching that resets the session.** Claude Code and Codex preserve prompt cache/conversation across `/model`; a model picker that reloads the page or clears the transcript will be avoided by users.
9. **Pastehole.** Hermes and Gemini both collapse large pastes to a preview line and expand on demand. A textarea that accepts a 500-line paste without warning pushes the transcript into unreadability and surprises the user on submit.
10. **Checkpoints without semantics.** Cursor separates "restore files" from "keep messages" explicitly. Any rewind feature that mixes conversation rollback with filesystem rollback needs the two axes separated in the UI, not bundled into one destructive button.
