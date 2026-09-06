"""Steward sidecar runtime: a persistent Python session speaking the
Steward sidecar JSON protocol over stdio.

Protocol (one JSON object per line):
  <- {"kind": "create_session", "session_id": ..., "language": "python", "working_dir": ...}
  <- {"kind": "execute_cell", "session_id": ..., "cell_id": ..., "source": ...}
  <- {"kind": "bridge_call", "session_id": ..., "method": ..., "arguments": {...}}
  <- {"kind": "cancel", "session_id": ...}
  <- {"kind": "snapshot", "session_id": ...}
  <- {"kind": "destroy", "session_id": ...}
  -> {"kind": "session_created" | "cell_completed" | "bridge_result" | "cancelled"
      | "snapshot" | "destroyed" | "error", ...}

Execution model: cells run via `exec` in a persistent namespace so variables
survive across cells (Task 14.2). Cells can set persistent variables with
`steward.set(name, value)` and read them with `steward.get(name)`. The
sidecar holds NO daemon capabilities: bridge calls go back to the host over
the protocol and the host enforces policy.
"""

from __future__ import annotations

import io
import json
import contextlib
import sys
import threading
from typing import Any

from steward_runtime.bridge import StewardBridge


class Session:
    """One persistent Python session with an isolated namespace."""

    def __init__(self, session_id: str, working_dir: str, host: Any) -> None:
        self.session_id = session_id
        self.working_dir = working_dir
        self.alive = True
        self.namespace: dict[str, Any] = {"__name__": "__steward_cell__"}
        self.lock = threading.Lock()
        self.bridge = StewardBridge(session_id, host)

    def execute(self, cell_id: str, source: str) -> dict[str, Any]:
        stdout = io.StringIO()
        stderr = io.StringIO()
        error = None
        with self.lock:
            if not self.alive:
                raise RuntimeError(f"session '{self.session_id}' is destroyed")
            self.namespace["steward"] = self.bridge
            try:
                with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                    result = eval(compile(source, f"<cell:{cell_id}>", "eval"), self.namespace)
                    if result is not None:
                        print(repr(result))
            except SyntaxError:
                try:
                    with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                        exec(compile(source, f"<cell:{cell_id}>", "exec"), self.namespace)
                except Exception as exc:  # noqa: BLE001 - cells are untrusted
                    error = f"{type(exc).__name__}: {exc}"
            except Exception as exc:  # noqa: BLE001
                error = f"{type(exc).__name__}: {exc}"

        return {
            "kind": "cell_completed",
            "cell_id": cell_id,
            "output": stdout.getvalue(),
            "stderr": stderr.getvalue(),
            "error": error,
            "variables": {k: repr(v) for k, v in self.namespace.items() if not k.startswith("__")},
        }

    def snapshot(self) -> dict[str, Any]:
        with self.lock:
            return {
                "kind": "snapshot",
                "variables": {k: repr(v) for k, v in self.namespace.items() if not k.startswith("__")},
            }


class Runtime:
    """All sessions owned by this sidecar process."""

    def __init__(self, host: Any) -> None:
        self.sessions: dict[str, Session] = {}
        self.host = host

    def handle(self, request: dict[str, Any]) -> dict[str, Any]:
        kind = request.get("kind")
        if kind == "create_session":
            session_id = request["session_id"]
            if session_id in self.sessions:
                raise RuntimeError(f"session '{session_id}' already exists")
            self.sessions[session_id] = Session(session_id, request.get("working_dir", "."), self.host)
            return {"kind": "session_created", "session_id": session_id}
        if kind == "execute_cell":
            session = self._require(request["session_id"])
            return session.execute(request["cell_id"], request["source"])
        if kind == "bridge_call":
            session = self._require(request["session_id"])
            result = session.bridge.call(request["method"], request.get("arguments", {}))
            return {"kind": "bridge_result", "method": request["method"], "result": result}
        if kind == "cancel":
            self._require(request["session_id"]).alive = False
            return {"kind": "cancelled"}
        if kind == "snapshot":
            return self._require(request["session_id"]).snapshot()
        if kind == "destroy":
            session_id = request["session_id"]
            if session_id not in self.sessions:
                raise RuntimeError(f"session '{session_id}' not found")
            del self.sessions[session_id]
            return {"kind": "destroyed"}
        raise RuntimeError(f"unknown request kind {kind!r}")

    def _require(self, session_id: str) -> Session:
        session = self.sessions.get(session_id)
        if session is None:
            raise RuntimeError(f"session '{session_id}' not found")
        return session


def main() -> int:  # pragma: no cover - thin IO loop
    runtime = Runtime(host=sys.stdin)  # bridge reads from the same wire
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            request = json.loads(line)
            response = runtime.handle(request)
        except Exception as exc:  # noqa: BLE001 - protocol errors go to the host
            response = {"kind": "error", "message": f"{type(exc).__name__}: {exc}"}
        sys.stdout.write(json.dumps(response) + "\n")
        sys.stdout.flush()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
