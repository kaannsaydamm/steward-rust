"""Steward daemon client for the Hermes gateway adapter.

Replaces the Hermes agent runtime (``run_agent.AIAgent``) at the exact
seam the gateway consumes: ``run_conversation(message, **kwargs)`` with
``stream_callback`` for deltas plus ``tool_start_callback`` /
``tool_complete_callback`` / ``tool_progress_callback`` hooks. Everything
else in the vendored gateway keeps speaking to this object.

Talks to the Steward daemon over gRPC (default 127.0.0.1:50051; the
gateway-side JSON-RPC surface is unchanged).

Provenance: adapter code, not upstream. Upstream contract documented in
docs/OSS_PORT_MAP.md (H4).
"""

from __future__ import annotations

import os
import sys
import time
from typing import Any, Callable

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "stubs"))

import grpc  # noqa: E402

import steward_pb2 as pb  # noqa: E402
import steward_pb2_grpc as pb_grpc  # noqa: E402

DEFAULT_ADDR = os.environ.get("STEWARD_DAEMON_ADDR", "127.0.0.1:50051")


class StewardGatewayAgent:
    """Drop-in stand-in for ``run_agent.AIAgent`` backed by the Steward daemon.

    Gateway touch points (verified against the vendored tree):
      - ``run_conversation(message, conversation_history=..., stream_callback=...,
        persist_user_message=..., task_id=...)`` → ``{"messages": [...], "session_id": ...}``
      - ``tool_start_callback / tool_complete_callback / tool_progress_callback``
      - ``session_id``, ``get_activity_summary()``, ``interim_assistant_callback``,
        ``_session_messages``, ``close()``
    """

    def __init__(
        self,
        addr: str = DEFAULT_ADDR,
        model: str | None = None,
        working_directory: str = "",
        allow_tools: bool = True,
    ) -> None:
        self._addr = addr
        self.model_override = model
        self.working_directory = working_directory
        self.allow_tools = allow_tools
        # The gateway assigns its own session key; Steward mints its own ids.
        # ``gateway_key`` is what the gateway sees, ``session_id`` mirrors the
        # Steward session once the first turn mints it (mapping below).
        self.gateway_key = ""
        self.session_id = ""
        self._sid_map: dict[str, str] = {}
        self._session_messages: list[dict] = []
        self.interim_assistant_callback: Callable[..., None] | None = None
        self.tool_start_callback: Callable[[str, str, dict], None] | None = None
        self.tool_complete_callback: Callable[[str, str, dict, str], None] | None = None
        self.tool_progress_callback: Callable[..., None] | None = None
        self._last_activity_ns = time.monotonic_ns()
        self._closed = False
        self._channel = grpc.insecure_channel(addr)
        self._stub = pb_grpc.StewardServiceStub(self._channel)

    # -- lifecycle ---------------------------------------------------------

    def close(self) -> None:
        self._closed = True
        self._channel.close()

    def get_activity_summary(self) -> dict:
        now = time.monotonic_ns()
        elapsed = (now - self._last_activity_ns) / 1_000_000_000
        return {"last_activity_at": now, "seconds_since_activity": elapsed}

    def _touch(self) -> None:
        self._last_activity_ns = time.monotonic_ns()

    # -- the turn seam -----------------------------------------------------

    def run_conversation(
        self,
        user_message: Any,
        *,
        conversation_history: list | None = None,
        stream_callback: Callable[[str], None] | None = None,
        persist_user_message: Any = None,
        task_id: str = "",
        **_ignored: Any,
    ) -> dict:
        """One turn: user text in, streamed answer out. Blocks until done."""
        if self._closed:
            raise RuntimeError("StewardGatewayAgent is closed")
        self._touch()
        message = user_message if isinstance(user_message, str) else str(user_message)
        if isinstance(persist_user_message, str) and persist_user_message:
            message = persist_user_message

        # Gateway key → Steward session mapping: the first turn lets Steward
        # mint the session (empty session_id); later turns resume it.
        key = self.gateway_key or task_id
        steward_sid = self._sid_map.get(key, "")

        self._session_messages.append({"role": "user", "content": message})

        try:
            stream = self._stub.Chat(
                pb.ChatRequest(
                    session_id=steward_sid,
                    message=message,
                    working_directory=self.working_directory,
                    allow_tools=self.allow_tools,
                ),
                timeout=600.0,
            )
            for event in stream:
                self._touch()
                kind = event.kind
                if kind == pb.CHAT_EVENT_KIND_SESSION:
                    if event.session_id:
                        self.session_id = event.session_id
                        if key:
                            self._sid_map[key] = event.session_id
                elif kind == pb.CHAT_EVENT_KIND_TEXT:
                    if stream_callback is not None and event.content:
                        stream_callback(event.content)
                    self._session_messages.append(
                        {"role": "assistant", "content": event.content}
                    )
                elif kind in (pb.CHAT_EVENT_KIND_TOOL_START, pb.CHAT_EVENT_KIND_TOOL_RESULT):
                    if kind == pb.CHAT_EVENT_KIND_TOOL_START:
                        if self.tool_start_callback is not None:
                            self.tool_start_callback(event.tool_call_id or "call", event.tool_name, {})
                    else:
                        if self.tool_complete_callback is not None:
                            self.tool_complete_callback(
                                event.tool_call_id or "call", event.tool_name, {}, event.content
                            )
                    self._session_messages.append(
                        {"role": "tool", "content": event.content, "tool_name": event.tool_name}
                    )
        except grpc.RpcError as exc:
            self._touch()
            return {
                "messages": self._session_messages,
                "session_id": self.session_id,
                "status": "error",
                "error": str(exc),
            }

        return {
            "messages": self._session_messages,
            "session_id": self.session_id,
            "status": "complete",
        }

    # -- session info for the gateway's session.info mirror -----------------

    def session_info(self) -> dict:
        return {
            "session_id": self.session_id,
            "model": self.model_override or "",
            "provider": "steward",
            "backend": "steward-daemon",
        }

    # -- side-question support (prompt.btw snapshot path) -------------------

    def answer_side_question(self, question: str) -> str:
        """Synchronous one-shot completion, history untouched."""
        try:
            reply = self._stub.Chat(
                pb.ChatRequest(
                    session_id="",
                    message=question,
                    working_directory=self.working_directory,
                    allow_tools=False,
                ),
                timeout=120.0,
            )
            return "".join(e.content for e in reply if e.kind == pb.CHAT_EVENT_KIND_TEXT)
        except grpc.RpcError as exc:
            return f"(steward side question failed: {exc})"


def ping(addr: str = DEFAULT_ADDR, timeout: float = 5.0) -> str:
    """Health check used by the bridge smoke test."""
    channel = grpc.insecure_channel(addr)
    stub = pb_grpc.StewardServiceStub(channel)
    try:
        response = stub.Ping(pb.PingRequest(), timeout=timeout)
        return response.status or "OK"
    finally:
        channel.close()
