# Vendored: microsoft/autogen — agent builder (autogen-studio)

- Source: https://github.com/microsoft/autogen
- Pinned SHA: 027ecf0a379bcc1d09956d46d12d44a3ad9cee14
- License: code MIT via LICENSE-CODE (© Microsoft Corporation); docs/prose CC-BY-4.0 (excluded)
- Vendored paths:
  - `autogenstudio/` — FastAPI backend: routes (`teams`, `validate`), datamodel,
    validation service, teammanager, database layer
  - `frontend/` — Gatsby/React teambuilder: graph editor (xyflow), component
    editor, library, validation UI (`builder/builder.tsx` + 25 component files)
  - `pyproject.toml` — backend dependencies (fastapi, sqlmodel, pydantic…)
- Adapter boundary (Steward side, outside this tree):
  - `builder/shim/` — thin FastAPI shim exposing the studio `/teams` + `/validate`
    contract backed by Steward daemon `SaveAgentProfile`/`ListAgentProfiles` RPCs
  - team component config → `AgentProfile` YAML mapping
- Sync: mirrors upstream paths at the pinned SHA
