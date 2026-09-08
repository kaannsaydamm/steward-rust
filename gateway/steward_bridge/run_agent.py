"""Steward adapter module masquerading as Hermes' ``run_agent``.

The vendored gateway imports ``from run_agent import AIAgent`` at three
sites (methods_prompt.py ×2, server.py ×1). Instead of patching those
upstream files, this package is placed on ``sys.path[0]`` as ``run_agent``
by the Steward launcher (``steward-gateway-bridge``), so the import
resolves here and every construction site gets a Steward-backed agent.

Constructor kwargs upstream passes (model/provider/base_url/api_key/...)
are accepted and the relevant ones forwarded to the Steward daemon; the
rest are ignored — the daemon owns model truth.

This is ADAPTER code (docs/OSS_PORT_MAP.md H4), not upstream.
"""

from __future__ import annotations

import json
import os
import sys
import urllib.parse
from typing import Any

_BRIDGE_DIR = os.path.dirname(os.path.abspath(__file__))
if _BRIDGE_DIR not in sys.path:
    sys.path.insert(0, _BRIDGE_DIR)

from steward_agent import StewardGatewayAgent  # noqa: E402


def _cwd_from_session_id(session_id: str | None) -> str:
    """Sessions may carry ``<sid>@<cwd>`` style keys upstream; extract a cwd
    when one is embedded, else empty (daemon default)."""
    if not session_id or "@" not in session_id:
        return ""
    candidate = session_id.rsplit("@", 1)[1]
    return candidate if os.path.isdir(candidate) else ""


class AIAgent(StewardGatewayAgent):
    """Gateway-compatible agent backed by the Steward daemon.

    Accepts the full upstream ``AIAgent(**kwargs)`` constructor surface;
    forwards model/working_directory/session identity and swallows the
    Hermes-runtime-specific knobs the daemon does not own.
    """

    def __init__(self, **kwargs: Any) -> None:
        model = kwargs.get("model")
        session_key = kwargs.get("session_id") or ""
        # explicit cwd override wins; else derive from session key
        cwd = kwargs.get("working_directory") or _cwd_from_session_id(session_key)
        # normalize a session id that embedded a cwd
        clean_session = session_key.split("@", 1)[0] if "@" in session_key else session_key
        super().__init__(
            model=model if isinstance(model, str) else None,
            working_directory=cwd,
            allow_tools=bool(kwargs.get("allow_tools", True)),
        )
        # NOTE: gateway session key does NOT become the Steward session id;
        # the daemon mints its own id on the first Chat and the base class
        # maps key → steward id internally.
        # upstream agents carry a session db handle; the daemon persists its
        # own state, so keep a no-op placeholder for attribute access sites.
        if clean_session:
            self.gateway_key = clean_session
        self._on_session_title = None

    def run_conversation(self, user_message: Any, **kwargs: Any) -> dict:  # noqa: D102
        result = super().run_conversation(user_message, **kwargs)
        # upstream result contract extras the gateway reads
        result.setdefault("raw_response", result.get("messages"))
        return result


# ── Agent-profile proxy methods ─────────────────────────────────────────
# The gateway imports this module before handing control to tui_gateway, so
# registering into its _methods table here extends the JSON-RPC surface with
# zero upstream edits. Desktop/Web Agent Builder pages call these.

def _profiles_stub():
    import grpc  # noqa: E402
    import steward_pb2 as pb  # noqa: E402
    import steward_pb2_grpc as pb_grpc  # noqa: E402
    addr = os.environ.get("STEWARD_DAEMON_ADDR", "127.0.0.1:50051")
    return pb_grpc.StewardServiceStub(grpc.insecure_channel(addr)), pb


def _profile_to_dict(p) -> dict:
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


def _register_profile_methods() -> None:
    try:
        from tui_gateway.server import _methods, _err, _ok  # type: ignore
    except Exception:
        return  # not running inside the gateway process

    def list_profiles(rid, _params):
        stub, pb = _profiles_stub()
        resp = stub.ListAgentProfiles(pb.ListAgentProfilesRequest(), timeout=30.0)
        return _ok(rid, {"profiles": [_profile_to_dict(p) for p in resp.profiles]})

    def save_profile(rid, params):
        yaml_text = str((params or {}).get("yaml", ""))
        if not yaml_text.strip():
            return _err(rid, 4002, "yaml required")
        stub, pb = _profiles_stub()
        # The rpc returns AgentProfileInfo directly (not a wrapper).
        resp = stub.SaveAgentProfile(pb.SaveAgentProfileRequest(yaml=yaml_text), timeout=30.0)
        if resp.error:
            return _err(rid, 4003, resp.error)
        return _ok(rid, {"profile": _profile_to_dict(resp)})

    def delete_profile(rid, params):
        pid = str((params or {}).get("id", ""))
        if not pid:
            return _err(rid, 4002, "id required")
        stub, pb = _profiles_stub()
        resp = stub.DeleteAgentProfile(pb.DeleteAgentProfileRequest(id=pid), timeout=30.0)
        return _ok(rid, {"deleted": bool(resp.deleted)})

    def export_profile(rid, params):
        pid = str((params or {}).get("id", ""))
        if not pid:
            return _err(rid, 4002, "id required")
        stub, pb = _profiles_stub()
        resp = stub.ExportAgentProfile(pb.ExportAgentProfileRequest(id=pid), timeout=30.0)
        return _ok(rid, {"yaml": resp.yaml})

    def kg_graph(rid, params):
        """Knowledge graph snapshot (nodes/edges) via the daemon's /api/kg
        HTTP surface — the web dashboard renders this as the KG viewer."""
        import urllib.request  # noqa: E402
        port_file = os.path.expanduser("~/.steward/wire-port")
        try:
            port = open(port_file, encoding="utf-8").read().strip()
        except OSError as exc:
            return _err(rid, 5001, f"wire port unreadable: {exc}")
        depth = int((params or {}).get("depth", 2))
        filt = str((params or {}).get("filter", ""))
        url = f"http://127.0.0.1:{port}/api/kg/graph?depth={depth}&filter={urllib.parse.quote(filt)}"
        try:
            with urllib.request.urlopen(url, timeout=30) as res:
                payload = json.loads(res.read().decode("utf-8"))
        except Exception as exc:  # noqa: BLE001 — surface any failure to the client
            return _err(rid, 5002, str(exc))
        return _ok(rid, payload)

    def kg_query(rid, params):
        import urllib.request  # noqa: E402
        port_file = os.path.expanduser("~/.steward/wire-port")
        try:
            port = open(port_file, encoding="utf-8").read().strip()
        except OSError as exc:
            return _err(rid, 5001, f"wire port unreadable: {exc}")
        body = json.dumps({
            "query": str((params or {}).get("query", "")),
            "max_hops": int((params or {}).get("max_hops", 2)),
            "limit": int((params or {}).get("limit", 50)),
        }).encode("utf-8")
        url = f"http://127.0.0.1:{port}/api/kg/query"
        req = urllib.request.Request(url, data=body, headers={"Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(req, timeout=30) as res:
                payload = json.loads(res.read().decode("utf-8"))
        except Exception as exc:  # noqa: BLE001
            return _err(rid, 5002, str(exc))
        return _ok(rid, payload)

    _methods.setdefault("steward.profiles.list", list_profiles)
    _methods.setdefault("steward.profiles.save", save_profile)
    _methods.setdefault("steward.profiles.delete", delete_profile)
    _methods.setdefault("steward.profiles.export", export_profile)
    _methods.setdefault("steward.kg.graph", kg_graph)
    _methods.setdefault("steward.kg.query", kg_query)


_register_profile_methods()
