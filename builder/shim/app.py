"""Steward shim for the vendored autogen-studio builder frontend.

Exposes the studio's `/api/teams` + `/api/validate` REST contract backed by
the **Steward daemon** RPC surface (SaveAgentProfile / ListAgentProfiles /
DeleteAgentProfile) instead of the upstream autogenstudio database. Team
component configs map to AgentProfile YAML (persona/skills/subagents/
template); validation reuses the upstream ValidationService for schema
checks and additionally checks AgentProfile-constructibility via the
mapping in `profile_mapping.py`.

Run:
    uvicorn builder.shim.app:app --port 8123

Then point the studio frontend at it: GATSBY_API_URL=http://127.0.0.1:8123/api

ADAPTER code — docs/OSS_PORT_MAP.md A1/A2. Upstream contract unchanged;
all agent truth lives in the Steward daemon.
"""

from __future__ import annotations

import os
import sys
import uuid
from typing import Any, Dict, List, Optional

# Make the vendored studio package importable (datamodel, validation services).
_HERE = os.path.dirname(os.path.abspath(__file__))
_VENDOR_STUDIO = os.path.abspath(os.path.join(_HERE, ".."))
if _VENDOR_STUDIO not in sys.path:
    sys.path.insert(0, _VENDOR_STUDIO)

# Bridge dir supplies the generated gRPC stubs.
_BRIDGE = os.path.abspath(os.path.join(_HERE, "..", "..", "gateway", "steward_bridge"))
if _BRIDGE not in sys.path:
    sys.path.insert(0, _BRIDGE)

import grpc  # noqa: E402
import steward_pb2 as pb  # noqa: E402
import steward_pb2_grpc as pb_grpc  # noqa: E402
from fastapi import APIRouter, FastAPI, HTTPException  # noqa: E402
from fastapi.middleware.cors import CORSMiddleware  # noqa: E402
from pydantic import BaseModel  # noqa: E402

try:
    from .profile_mapping import component_to_agent_profile_yaml, agent_profile_error
except ImportError:  # executed as a script/module without package context
    from profile_mapping import component_to_agent_profile_yaml, agent_profile_error

DAEMON_ADDR = os.environ.get("STEWARD_DAEMON_ADDR", "127.0.0.1:50051")


def _stub() -> "pb_grpc.StewardServiceStub":
    channel = grpc.insecure_channel(DAEMON_ADDR)
    return pb_grpc.StewardServiceStub(channel)


class _Response(BaseModel):
    status: bool
    data: Optional[List[Dict[str, Any]]] = None
    message: Optional[str] = None


# In-memory team store: team_id -> {user_id, component_config, profile_id}.
# The durable truth is the AgentProfile the daemon persists on save.
_TEAMS: Dict[int, Dict[str, Any]] = {}
_NEXT_ID = [1]


def _config_to_label(config: Dict[str, Any]) -> str:
    label = config.get("label") or config.get("name") or "team"
    return str(label)


def _save_profile(component_config: Dict[str, Any]) -> tuple[str, str]:
    """Persists the mapped AgentProfile via the daemon; returns (profile_id, yaml)."""
    yaml_text = component_to_agent_profile_yaml(component_config)
    error = agent_profile_error(yaml_text)
    if error:
        raise HTTPException(status_code=400, detail=error)
    stub = _stub()
    response = stub.SaveAgentProfile(
        pb.SaveAgentProfileRequest(yaml=yaml_text), timeout=30
    )
    if response.error:
        raise HTTPException(status_code=400, detail=response.error)
    return response.id, yaml_text


router = APIRouter()


@router.get("/teams/")
async def list_teams(user_id: str = "default") -> Dict[str, Any]:
    data = [
        {
            "id": team_id,
            "user_id": team["user_id"],
            "component": team["component"],
        }
        for team_id, team in _TEAMS.items()
        if team["user_id"] == user_id
    ]
    return {"status": True, "data": data}


@router.get("/teams/{team_id}")
async def get_team(team_id: int, user_id: str = "default") -> Dict[str, Any]:
    team = _TEAMS.get(team_id)
    if not team or team["user_id"] != user_id:
        raise HTTPException(status_code=404, detail="Team not found")
    return {"status": True, "data": [dict(team, id=team_id)]}


@router.post("/teams/")
async def create_team(team: Dict[str, Any]) -> Dict[str, Any]:
    component = team.get("component") or {}
    user_id = team.get("user_id", "default")
    profile_id, _yaml = _save_profile(component)
    team_id = _NEXT_ID[0]
    _NEXT_ID[0] += 1
    _TEAMS[team_id] = {
        "user_id": user_id,
        "component": component,
        "profile_id": profile_id,
    }
    return {"status": True, "data": [dict(_TEAMS[team_id], id=team_id)]}


@router.put("/teams/{team_id}")
async def update_team(team_id: int, team: Dict[str, Any]) -> Dict[str, Any]:
    existing = _TEAMS.get(team_id)
    if not existing:
        raise HTTPException(status_code=404, detail="Team not found")
    component = team.get("component") or existing["component"]
    profile_id, _yaml = _save_profile(component)
    existing["component"] = component
    existing["profile_id"] = profile_id
    return {"status": True, "data": [dict(existing, id=team_id)]}


@router.delete("/teams/{team_id}")
async def delete_team(team_id: int, user_id: str = "default") -> Dict[str, Any]:
    team = _TEAMS.pop(team_id, None)
    if not team or team["user_id"] != user_id:
        return {"status": True, "message": "Team deleted successfully"}
    stub = _stub()
    stub.DeleteAgentProfile(
        pb.DeleteAgentProfileRequest(id=team["profile_id"]), timeout=30
    )
    return {"status": True, "message": "Team deleted successfully"}


@router.post("/validate/")
async def validate_component(component: Dict[str, Any]) -> Dict[str, Any]:
    """Schema validation via the upstream ValidationService plus a Steward
    AgentProfile mapping check (the kernel must be able to construct it)."""
    errors: List[Dict[str, str]] = []
    warnings: List[Dict[str, str]] = []
    try:
        from autogenstudio.validation.validation_service import ValidationService

        result = ValidationService.validate(component)
        errors.extend({"field": e.field, "error": e.error} for e in result.errors)
        warnings.extend({"field": w.field, "error": w.error} for w in result.warnings)
    except Exception as exc:  # upstream service optional at runtime
        warnings.append({"field": "upstream", "error": str(exc)})

    mapping_error = agent_profile_error(component_to_agent_profile_yaml(component))
    if mapping_error:
        errors.append({"field": "steward_profile", "error": mapping_error})

    return {"is_valid": not errors, "errors": errors, "warnings": warnings}


app = FastAPI(title="Steward Builder Shim")
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_methods=["*"],
    allow_headers=["*"],
)
app.include_router(router, prefix="/api")
