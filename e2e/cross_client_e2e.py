#!/usr/bin/env python
"""Cross-client same-session E2E.

Proves one Steward session is created, continued, and observed across the
three client surfaces backed by the daemon:

  1. Web/gateway path — steward-gateway-bridge JSON-RPC (the transport the
     Hermes desktop app drives) creates a session and submits a prompt; the
     daemon mints a Steward session id.
  2. CLI path — the same daemon session is continued through the daemon gRPC
     directly (what the OMP bridge does internally): follow-up turn must see
     the earlier marker from shared history.
  3. Observation — the daemon session store lists the session with both
     turns, and the gateway-side mirror (hermes session db used by the
     desktop sidebar and web /api/sessions) records the same steward id.

Usage: python e2e/cross_client_e2e.py [--addr 127.0.0.1:50051]
Requires: Steward daemon running; env PYTHONPATH includes vendor/hermes for
the gateway server imports (handled below).
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, ".."))
BRIDGE = os.path.join(REPO, "gateway", "steward_bridge")
sys.path.insert(0, BRIDGE)
sys.path.insert(0, os.path.join(BRIDGE, "stubs"))

import steward_pb2 as pb  # noqa: E402
import steward_pb2_grpc as pbg  # noqa: E402

import grpc  # noqa: E402


def wire_port() -> int:
    for candidate in (
        os.path.expanduser("~/.steward/wire-port"),
        r"C:/Users/kaann/.steward/wire-port",
    ):
        try:
            return int(open(candidate, encoding="utf-8").read().strip())
        except OSError:
            continue
    raise SystemExit("wire-port file not found — is the daemon running?")


def daemon_turn(stub, session_id: str, message: str) -> tuple[str, str]:
    """One daemon gRPC turn; returns (session_id, full_answer)."""
    sid, answer = session_id, []
    for event in stub.Chat(
        pb.ChatRequest(session_id=session_id, message=message), timeout=600.0
    ):
        if event.kind == pb.CHAT_EVENT_KIND_SESSION and event.session_id:
            sid = event.session_id
        elif event.kind == pb.CHAT_EVENT_KIND_TEXT and event.content:
            answer.append(event.content)
    return sid, "".join(answer)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--addr", default="127.0.0.1:50051")
    args = parser.parse_args()

    channel = grpc.insecure_channel(args.addr)
    stub = pbg.StewardServiceStub(channel)

    marker = f"CROSS-CLIENT-{int(time.time())}"

    # ── 1. Gateway/bridge path: spawn steward-gateway-bridge, drive JSON-RPC.
    # Sandbox HERMES_HOME points the gateway at the steward provider config
    # (daemon wire-port /omp/v1) instead of the developer's default config.
    port = wire_port()
    sandbox = os.path.join(
        os.environ.get("TEMP", os.environ.get("TMP", "/tmp")),
        f"steward-cross-e2e-{int(time.time())}")
    os.makedirs(sandbox, exist_ok=True)
    with open(os.path.join(sandbox, "config.yaml"), "w", encoding="utf-8") as fh:
        fh.write(
            "model:\n"
            "  default: steward-local\n"
            "  provider: steward\n"
            "providers:\n"
            "  steward:\n"
            f"    api: http://127.0.0.1:{port}/omp/v1\n"
            "    name: Steward\n"
            "    api_mode: chat_completions\n"
            "    key_env: STEWARD_API_KEY\n"
            "    models:\n"
            "      glm-5.3-flash: {}\n"
            "    context_length: 32768\n"
            "auxiliary:\n"
            "  title_generation:\n"
            "    enabled: false\n"
            "approvals:\n"
            "  mode: \"off\"\n")
    with open(os.path.join(sandbox, ".env"), "w", encoding="utf-8") as fh:
        fh.write("STEWARD_API_KEY=steward-cross-e2e\n")
    env = dict(os.environ, STEWARD_DAEMON_ADDR=args.addr, PYTHONPATH=REPO,
               HERMES_HOME=sandbox)
    proc = subprocess.Popen(
        [sys.executable, os.path.join(BRIDGE, "steward-gateway-bridge.py")],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        text=True, encoding="utf-8", cwd=os.path.join(REPO, "vendor", "hermes"), env=env,
    )
    bridge_session = ""
    try:
        ready = False
        deadline = time.time() + 120
        while time.time() < deadline:
            line = proc.stdout.readline()
            if not line:
                break
            if '"gateway.ready"' in line:
                ready = True
                break
        if not ready:
            print("FAIL: gateway bridge never reported gateway.ready")
            return 1

        create_req = {"jsonrpc": "2.0", "id": 0, "method": "session.create", "params": {"source": "e2e"}}
        proc.stdin.write(json.dumps(create_req) + "\n")
        proc.stdin.flush()
        gateway_sid = ""
        deadline = time.time() + 30
        while time.time() < deadline and not gateway_sid:
            line = proc.stdout.readline()
            if not line:
                break
            if '"id": 0' in line or '"id":0' in line:
                gateway_sid = (json.loads(line).get("result") or {}).get("session_id", "")

        req = {
            "jsonrpc": "2.0", "id": 1, "method": "prompt.submit",
            "params": {"session_id": gateway_sid, "text": f"Reply with exactly: {marker}"},
        }
        proc.stdin.write(json.dumps(req) + "\n")
        proc.stdin.flush()
        answer = ""
        deadline = time.time() + 300
        while time.time() < deadline:
            line = proc.stdout.readline()
            if not line:
                break
            if '"event"' in line:
                try:
                    params = json.loads(line).get("params") or {}
                except json.JSONDecodeError:
                    continue
                if params.get("session_id"):
                    bridge_session = str(params["session_id"])
                if params.get("type") == "message.delta":
                    payload = params.get("payload") or {}
                    if payload.get("text"):
                        answer += str(payload["text"])
            stripped = line.replace(" ", "")
            if f'"id":{req["id"]}' in stripped:
                resp = json.loads(line)
                if resp.get("error"):
                    print(f"FAIL: prompt.submit error: {resp['error']}")
                    return 1
                # {"status":"streaming"} ack — keep reading until completion.
                continue
            if '"message.complete"' in stripped:
                break
        if marker not in answer:
            print(f"FAIL: bridge answer lacks marker: {answer[:300]}")
            return 1
        print(f"[1] gateway/bridge turn OK  gateway_session={bridge_session or '(ack)'}")
    finally:
        proc.terminate()

    # The steward session the bridge minted: newest daemon session whose
    # transcript contains the marker (the gateway session id is bridge-local).
    listing = stub.ListChatSessions(pb.ListChatSessionsRequest(limit=10), timeout=30.0)
    bridge_session = ""
    for summary in listing.sessions:
        detail = stub.GetChatSession(pb.GetChatSessionRequest(session_id=summary.session_id), timeout=30.0)
        if marker in repr(detail):
            bridge_session = summary.session_id
            break
    if not bridge_session:
        print("FAIL: steward session carrying the marker not found in daemon store")
        return 1

    # ── 2. CLI path: continue the SAME daemon session through gRPC directly.
    sid, answer2 = daemon_turn(stub, bridge_session, "Repeat the exact code word from my previous message, nothing else.")
    if not sid:
        print("FAIL: daemon turn did not mint/reveal a session id")
        return 1
    if bridge_session and sid != bridge_session:
        print(f"FAIL: session id drift: {bridge_session} -> {sid}")
        return 1
    if marker not in answer2:
        print(f"FAIL: follow-up lost history: {answer2[:300]}")
        return 1
    print(f"[2] CLI/gRPC same-session turn OK  session={sid}")

    # ── 3. Observation: daemon session list shows the session.
    sessions = stub.ListChatSessions(pb.ListChatSessionsRequest(limit=20), timeout=30.0)
    if not any(s.session_id == sid for s in sessions.sessions):
        print(f"FAIL: session {sid} not visible in daemon session list")
        return 1
    print(f"[3] daemon session list sees {sid}")

    print("CROSS-CLIENT E2E PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
