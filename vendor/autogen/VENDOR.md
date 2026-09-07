# Vendored: microsoft/autogen (agent builder)

- Source: https://github.com/microsoft/autogen
- Pinned SHA: 027ecf0a379bcc1d09956d46d12d44a3ad9cee14
- License: code MIT via LICENSE-CODE (© Microsoft Corporation); docs/prose CC-BY-4.0 (excluded from vendor)
- Vendored paths:
  - `autogen-studio/` — builder backend (FastAPI, datamodel, validation, teammanager) + frontend (Gatsby/React teambuilder: graph editor, component editor, validation)
  - `autogen-agentchat/` — declarative team configs (round-robin/selector/handoff)
  - `autogen-core/` — component model (config schema base)
- Local modifications: none yet; REST shim to Steward AgentProfile RPCs lands outside this tree
- Sync: mirrors upstream paths at the pinned SHA
