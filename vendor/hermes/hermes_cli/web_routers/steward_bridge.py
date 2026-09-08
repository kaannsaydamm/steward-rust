"""Steward AgentProfile dashboard routes — REST proxy over the Steward daemon's
gRPC agent-profile surface (``steward.StewardService`` ListAgentProfiles /
SaveAgentProfile / DeleteAgentProfile).

The web dashboard has no JSON-RPC gateway connection (that is the desktop's
transport), so the builder page talks plain REST to this router, which calls
the daemon directly over gRPC using the generated stubs from the
``gateway/steward_bridge`` adapter (same stubs the JSON-RPC proxy in
``gateway/steward_bridge/run_agent.py`` registers as ``steward.profiles.*``).

The daemon is authoritative: profiles live as ``~/.steward/agents/<id>.yaml``
(daemon store), shared by the CLI, the desktop builder and this web builder.

ADAPTER code (docs/OSS_PORT_MAP.md H4 family) — not upstream.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path
from typing import Any, Dict, List

from fastapi import APIRouter, HTTPException
from pydantic import BaseModel

router = APIRouter()

# Regenerated proto stubs shipped with the steward gateway bridge adapter.
_STEWARD_BRIDGE_ROOT = Path(__file__).resolve().parents[4] / "gateway" / "steward_bridge"
_STUBS_DIR = _STEWARD_BRIDGE_ROOT / "stubs"

_DAEMON_ADDR_DEFAULT = "127.0.0.1:50051"


def _daemon_addr() -> str:
    return os.environ.get("STEWARD_DAEMON_ADDR", "").strip() or _DAEMON_ADDR_DEFAULT


def _profiles_stub():
    """(StewardServiceStub, pb2) against the daemon; imports resolve lazily so a
    dashboard process without grpcio still boots (only these routes fail)."""
    if str(_STUBS_DIR) not in sys.path:
        sys.path.insert(0, str(_STUBS_DIR))
    try:
        import grpc  # noqa: E402
        import steward_pb2 as pb  # noqa: E402
        import steward_pb2_grpc as pb_grpc  # noqa: E402
    except ImportError as exc:
        raise HTTPException(
            status_code=500,
            detail=f"steward gRPC stubs unavailable: {exc}",
        )
    return pb_grpc.StewardServiceStub(grpc.insecure_channel(_daemon_addr())), pb


def _profile_to_dict(p) -> Dict[str, Any]:
    return {
        "id": p.id,
        "display_name": p.display_name,
        "template": p.template,
        "model_policy": p.model_policy,
        "yaml": p.yaml,
        "skills": list(p.skills),
        "subagents": list(p.subagents),
        **({"error": p.error} if p.error else {}),
    }


class ProfileSave(BaseModel):
    yaml: str


@router.get("/api/steward/profiles")
def list_steward_profiles() -> Dict[str, List[Dict[str, Any]]]:
    """All AgentProfiles from the daemon store (``~/.steward/agents/*.yaml``)."""
    stub, pb = _profiles_stub()
    try:
        resp = stub.ListAgentProfiles(pb.ListAgentProfilesRequest(), timeout=30.0)
    except Exception as exc:  # noqa: BLE001 — transport errors become 502
        raise HTTPException(status_code=502, detail=f"steward daemon unreachable: {exc}")
    return {"profiles": [_profile_to_dict(p) for p in resp.profiles]}


@router.post("/api/steward/profiles")
def save_steward_profile(body: ProfileSave) -> Dict[str, Any]:
    """Validate + persist one schema_version-1 profile YAML in the daemon."""
    if not body.yaml.strip():
        raise HTTPException(status_code=400, detail="yaml required")
    stub, pb = _profiles_stub()
    # Save returns AgentProfileInfo directly (not a wrapper) — daemon-side
    # validation failures surface via the ``error`` field (see agent_profiles.rs).
    try:
        resp = stub.SaveAgentProfile(pb.SaveAgentProfileRequest(yaml=body.yaml), timeout=30.0)
    except Exception as exc:  # noqa: BLE001
        raise HTTPException(status_code=502, detail=f"steward daemon unreachable: {exc}")
    if resp.error:
        raise HTTPException(status_code=400, detail=resp.error)
    return {"profile": _profile_to_dict(resp)}


@router.delete("/api/steward/profiles/{profile_id}")
def delete_steward_profile(profile_id: str) -> Dict[str, Any]:
    stub, pb = _profiles_stub()
    try:
        resp = stub.DeleteAgentProfile(pb.DeleteAgentProfileRequest(id=profile_id), timeout=30.0)
    except Exception as exc:  # noqa: BLE001
        raise HTTPException(status_code=502, detail=f"steward daemon unreachable: {exc}")
    return {"deleted": bool(resp.deleted), "id": profile_id}
