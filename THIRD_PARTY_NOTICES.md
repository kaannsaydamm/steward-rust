# Third-Party Notices

Upstream source is integrated in two modes:

1. **Vendored literal trees** - upstream source imported verbatim under
   `vendor/` at the pinned revision, kept unmodified (adapter/config seams
   live outside the tree):
   - `vendor/hermes` <- NousResearch/hermes-agent (desktop app, web UI,
     gateway, ui-tui, hermes_cli runtime) - Steward Desktop & Web surfaces
   - `vendor/omp` <- can1357/oh-my-pi (CLI/TUI runtime) - Steward CLI surface
   - `vendor/prime` <- PrimeIntellect-ai/prime-agent (RLM runtime) - Steward
     RLM subsystem source reference for the Rust kernel port
   - `vendor/autogen` <- microsoft/autogen (autogen-studio builder) - Steward
     Agent Builder surface
   Steward-side adapters that bind the vendored trees to the daemon live
   outside `vendor/`: `gateway/steward_bridge/` (Hermes gateway), the
   daemon `/omp/v1` OpenAI-compatible endpoint
   (`crates/daemon/src/app_server/omp_bridge.rs`), and `e2e/` harnesses.

2. **Semantic ports** - behavior and UX reimplemented on Steward's own
   backend in Rust/TypeScript, with provenance headers on files that carry
   adapted code. All four upstream projects are MIT-licensed; license texts
   are reproduced below.

## NousResearch/hermes-agent

- Repository: https://github.com/NousResearch/hermes-agent
- Upstream revision: `693641aa8b4359c602283bdbbc14041e03bc47bc`
- License: MIT — Copyright (c) 2025 Nous Research
- Ported artifacts and Steward targets:

| Upstream path | Steward target | Modification |
|---|---|---|
| `ui-tui/src/app/slash/fuzzyScore.ts` | `web-ui/src/components/CommandPalette.tsx` | Tiered scoring (exact 0 / prefix 1 / substring 2, description tokens +3) adapted to `ChatCommand` shape; ranking + availability filtering merged into `rankCommands` |
| `ui-tui/src/app/slash/` (SlashPopover semantics) | `web-ui/src/components/CommandPalette.tsx`, `web-ui/src/components/ChatTab.tsx` | Palette rendering, keyboard nav, `/help` show-all, Esc dismiss adapted to Steward composer |
| `tui_gateway/` JSON-RPC surface (method inventory) | `web-ui/src/lib/gateway.ts`, `proto/steward.proto` | Typed gateway client mapped onto Steward Wire v2 WebSocket + daemon RPC |
| session-picker / boot-failure / onboarding overlays | `web-ui/src/components/` (overlays), `desktop/src-tauri` | UX semantics ported; rendered via Steward chat/daemon state |

## can1357/oh-my-pi

- Repository: https://github.com/can1357/oh-my-pi
- Upstream revision: `a1b254047d12e143b7c6011536e918c6c35c5906`
- License: MIT — Copyright (c) 2025 Mario Zechner
- Ported artifacts and Steward targets:

| Upstream path | Steward target | Modification |
|---|---|---|
| `packages/tui/src/autocomplete.ts` (interaction semantics) | `crates/cli/src/tui.rs` | Subset of autocomplete interaction (menu nav, Tab/Enter/Esc, `!` shell prefix, `@` path completion) reimplemented for ratatui |
| `packages/tui/src/fuzzy.ts` scoring idea | `crates/harness/src/slash.rs` | Shared slash-command registry + fuzzy scoring in Rust |
| tool inventory concept | `crates/tools/src/` | Tool additions modeled on omp coverage (sqlite.query, pdf.extract, checkpoint, think, todo) |

## PrimeIntellect-ai/prime-agent

- Repository: https://github.com/PrimeIntellect-ai/prime-agent
- Upstream revision: `844e85545af6858dcb3d6cfe42bbfcf2ca0be4e5`
- License: MIT — Copyright (c) 2026 Prime Intellect
- Ported artifacts and Steward targets:

| Upstream path | Steward target | Modification |
|---|---|---|
| `core/rlm-runtime.ts` | `crates/kernel/src/rlm.rs` | RLM run request/subagent registry/termination semantics ported to Rust kernel; child spawn via harness scheduler |
| refinement pipeline (`refinement.ts`) | `crates/harness/src/refine.rs` | RefinementKind (prompt/memory/skill/subagent) + HarnessEntry store with snapshot/rollback |
| `footer-data-provider.ts` split | daemon `GetFooterData` RPC + `crates/cli/src/tui.rs` footer | Data/model split preserved: daemon computes, CLI renders |

## microsoft/autogen

- Repository: https://github.com/microsoft/autogen
- Upstream revision: `027ecf0a379bcc1d09956d46d12d44a3ad9cee14`
- License: MIT — Copyright (c) Microsoft Corporation
- Ported artifacts and Steward targets:

| Upstream path | Steward target | Modification |
|---|---|---|
| autogen-studio teambuilder semantics | `web-ui/src/components/AgentBuilder.tsx` | Profile form + YAML preview mapped 1:1 onto `AgentProfile` fields |
| `_group_chat` topologies (round-robin/selector/handoff) | `crates/kernel/src/ir.rs` team node kinds | Scheduler executes via existing superstep machinery |
| agent/team config round-trip | `crates/harness/src/agent_profile.rs` | persona/hooks/skills/subagents/template fields; YAML import/export |

## Ported-file header template

Files that carry adapted upstream code carry a header comment:

```
// Ported from <upstream-repo>@<sha> <upstream-path> (MIT). Modified for Steward: <one-line>.
```

Clean adaptations (no copied code, upstream design reference only) note the
upstream design in a comment instead of the license header.

---

## Licenses

### MIT License (NousResearch/hermes-agent)

Copyright (c) 2025 Nous Research

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

### MIT License (can1357/oh-my-pi)

Copyright (c) 2025 Mario Zechner

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

### MIT License (PrimeIntellect-ai/prime-agent)

Copyright (c) 2026 Prime Intellect

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

### MIT License (microsoft/autogen)

Copyright (c) Microsoft Corporation

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
