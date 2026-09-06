"""Capability-limited bridge exposed to sidecar cells as `steward`.

Every method forwards over the sidecar wire to the daemon, which enforces
policy before acting. The bridge holds no local capabilities itself.
"""

from __future__ import annotations

from typing import Any


class StewardBridge:
    """SDK surface matching §27.2 (ctx.search/read, tools, agents, memory)."""

    def __init__(self, session_id: str, host: Any) -> None:
        self.session_id = session_id
        # `host.send` transmits a wire request and returns the response dict.
        # In the stdio runtime the host is the wire itself; in tests it is a
        # stub-free callable provided by the daemon harness.
        self._host = host

    def _call(self, method: str, arguments: dict[str, Any]) -> Any:
        request = {
            "kind": "bridge_call",
            "session_id": self.session_id,
            "method": method,
            "arguments": arguments,
        }
        response = self._host.send(json_dumps(request))
        if response.get("kind") == "error":
            raise RuntimeError(response.get("message", "bridge call failed"))
        return response.get("result")

    # --- context -----------------------------------------------------------
    def search(self, query: str, **filters: Any) -> Any:
        return self._call("ctx.search", {"query": query, **filters})

    def read(self, ref: str) -> Any:
        return self._call("ctx.read", {"ref": ref})

    # --- tools -------------------------------------------------------------
    def tools_search(self, query: str) -> Any:
        return self._call("tools.search", {"query": query})

    def tools_call(self, name: str, arguments: dict[str, Any]) -> Any:
        return self._call("tools.call", {"name": name, "arguments": arguments})

    # --- agents ------------------------------------------------------------
    def agents_spawn(self, profile_id: str, task: str, **options: Any) -> Any:
        return self._call("agents.spawn", {"profile": profile_id, "task": task, **options})

    def agents_map(self, profile_id: str, items: list[Any], task_template: str) -> Any:
        return self._call("agents.map", {"profile": profile_id, "items": items, "task": task_template})

    def agents_wait(self, ids: list[str]) -> Any:
        return self._call("agents.wait", {"ids": ids})

    # --- memory / artifacts / models ---------------------------------------
    def memory_search(self, query: str, **filters: Any) -> Any:
        return self._call("memory.search", {"query": query, **filters})

    def memory_propose(self, content: str, kind: str, scope: str) -> Any:
        return self._call("memory.propose", {"content": content, "kind": kind, "scope": scope})

    def artifact_create(self, kind: str, data: str, metadata: dict[str, Any]) -> Any:
        return self._call("artifacts.create", {"kind": kind, "data": data, "metadata": metadata})

    def models_call(self, route: str, messages: list[dict[str, str]]) -> Any:
        return self._call("models.call", {"route": route, "messages": messages})

    # --- persistent cell state (local to the session, no daemon round trip)
    _variables: dict[str, Any] = {}

    def set(self, name: str, value: Any) -> None:
        StewardBridge._variables[name] = value

    def get(self, name: str, default: Any = None) -> Any:
        return StewardBridge._variables.get(name, default)


def json_dumps(value: dict[str, Any]) -> str:
    import json

    return json.dumps(value)
