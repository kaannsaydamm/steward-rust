# BRIEFING — 2026-06-11T14:51:00+03:00

## Mission
Build the Advanced TUI for the Steward CLI using ratatui, completing 3 milestones.

## 🔒 My Identity
- Archetype: sub_orch
- Roles: orchestrator
- Working directory: c:\Users\kaluclu\Desktop\steward\.agents\tui_orchestrator
- Original parent: main agent
- Original parent conversation ID: aa7e22c7-d4b2-4206-9363-dd1ae73520d9

## 🔒 My Workflow
- **Pattern**: Project / Iteration Loop
- **Scope document**: c:\Users\kaluclu\Desktop\steward\.agents\tui_orchestrator\SCOPE.md
1. **Decompose**: Decomposed into 3 milestones in SCOPE.md.
2. **Dispatch & Execute**:
   - **Direct (iteration loop)**: Explorer → Worker → Reviewer → Gate
3. **On failure** (in this order):
   - Retry: nudge stuck agent or re-send task
   - Replace: spawn fresh agent with partial progress
   - Skip: proceed without (only if non-critical)
   - Redistribute: split stuck agent's remaining work
   - Redesign: re-partition decomposition
   - Escalate: report to parent
4. **Succession**: Self-succeed at 16 spawns.
- **Work items**:
  1. Ratatui Setup & Lifecycle [PLANNED]
  2. UI Components & Event Loop [PLANNED]
  3. gRPC Integration [PLANNED]
- **Current phase**: 1
- **Current focus**: Milestone 1

## 🔒 Key Constraints
- Never write code or run commands directly. Delegate to subagents.
- Never reuse a subagent after it has delivered its handoff — always spawn fresh
- Audit enforcement: If Forensic Auditor reports INTEGRITY VIOLATION, fail unconditionally.
- Start heartbeat cron (*/10 * * * *)

## Current Parent
- Conversation ID: aa7e22c7-d4b2-4206-9363-dd1ae73520d9
- Updated: 2026-06-11T14:51:00+03:00

## Key Decisions Made
- [None yet]

## Team Roster
| Agent | Type | Work Item | Status | Conv ID |
|-------|------|-----------|--------|---------|
| Explorer 1 | teamwork_preview_explorer | Milestone 1 | completed | 249b5b6b-51fb-451e-9f65-3c1beafc3d18 |
| Explorer 2 | teamwork_preview_explorer | Milestone 1 | completed | d76ddb8b-3e1f-4dd5-8042-9d206a3d23b8 |
| Explorer 3 | teamwork_preview_explorer | Milestone 1 | completed | 70f53c7d-d12f-456b-8755-f7a194d66652 |
| Worker 1   | teamwork_preview_worker | Milestone 1 | completed | ff4d9842-5f35-4396-8f00-6d6d6389534b |
| Reviewer 1 | teamwork_preview_reviewer | Milestone 1 | pending | d03a3435-2bcb-4a2a-8fb7-b3e3d55c8fab |
| Reviewer 2 | teamwork_preview_reviewer | Milestone 1 | pending | 5f9fba54-6148-4d36-8bb9-d7d2d6e353ee |
| Auditor 1  | teamwork_preview_auditor | Milestone 1 | pending | a27fc507-4e50-4a41-acdd-bdcf0845ee78 |

## Succession Status
- Succession required: no
- Spawn count: 7 / 16
- Pending subagents: d03a3435-2bcb-4a2a-8fb7-b3e3d55c8fab, 5f9fba54-6148-4d36-8bb9-d7d2d6e353ee, a27fc507-4e50-4a41-acdd-bdcf0845ee78
- Predecessor: none
- Successor: not yet spawned

## Active Timers
- Heartbeat cron: not started
- Safety timer: none
- On succession: kill all timers before spawning successor
- On context truncation: run `manage_task(Action="list")` — re-create if missing

## Artifact Index
- c:\Users\kaluclu\Desktop\steward\.agents\tui_orchestrator\SCOPE.md - Scope and milestones
- c:\Users\kaluclu\Desktop\steward\.agents\tui_orchestrator\progress.md - Progress tracking
