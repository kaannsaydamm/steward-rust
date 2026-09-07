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

import os
import sys
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
