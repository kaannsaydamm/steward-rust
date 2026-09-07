#!/usr/bin/env python
"""Cross-client same-session E2E.

One Steward session created through the gateway/bridge path (the desktop/web
transport), continued through the daemon gRPC (the CLI path), then observed
in the daemon session store.

Usage: python e2e/cross_client_e2e.py [--addr 127.0.0.1:50051]
Requires: Steward daemon running.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time

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
    raise SystemExit("wire-port file not found - is the daemon running?")


def daemon_turn(stub, session_id: str, message: str, attempts: int = 3) -> tuple[str, str]:
    """One daemon gRPC turn; returns (session_id, full_answer).
    Retries provider-side 429s (shared upstream model rate limit) with backoff."""
    last_error: Exception | None = None
    for attempt in range(attempts):
        if attempt:
            time.sleep(20 * attempt)
        try:
            sid, answer = session_id, []
            for event in stub.Chat(
                pb.ChatRequest(session_id=session_id, message=message), timeout=600.0
            ):
                if event.kind == pb.CHAT_EVENT_KIND_SESSION and event.session_id:
                    sid = event.session_id
                elif event.kind == pb.CHAT_EVENT_KIND_TEXT and event.content:
                    answer.append(event.content)
            return sid, "".join(answer)
        except grpc.RpcError as exc:
            last_error = exc
            if "429" not in str(exc):
                raise
    raise last_error  # type: ignore[misc]


def run_bridge_leg(args, marker: str) -> str | None:
    """One gateway/bridge attempt; returns the streamed answer or None when
    the provider 429s (caller retries with a fresh sandbox)."""
    port = wire_port()
    sandbox = os.path.join(
        os.environ.get("TEMP", os.environ.get("TMP", "/tmp")),
        f"steward-cross-e2e-{int(time.time())}")
    os.makedirs(sandbox, exist_ok=True)
    config = (
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
        'approvals:\n'
        '  mode: "off"\n'
    )
    with open(os.path.join(sandbox, "config.yaml"), "w", encoding="utf-8") as fh:
        fh.write(config)
    with open(os.path.join(sandbox, ".env"), "w", encoding="utf-8") as fh:
        fh.write("STEWARD_API_KEY=steward-cross-e2e\n")
    env = dict(os.environ, STEWARD_DAEMON_ADDR=args.addr, PYTHONPATH=REPO,
               HERMES_HOME=sandbox)
    proc = subprocess.Popen(
        [sys.executable, os.path.join(BRIDGE, "steward-gateway-bridge.py")],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        text=True, encoding="utf-8", cwd=os.path.join(REPO, "vendor", "hermes"), env=env,
    )
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
            return None

        create_req = {"jsonrpc": "2.0", "id": 0, "method": "session.create", "params": {"source": "e2e"}}
        proc.stdin.write(json.dumps(create_req) + "\n")
        proc.stdin.flush()
        gateway_sid = ""
        deadline = time.time() + 30
        while time.time() < deadline and not gateway_sid:
            line = proc.stdout.readline()
            if not line:
                break
            if '"id":0' in line.replace(" ", ""):
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
                if params.get("type") == "message.delta":
                    payload = params.get("payload") or {}
                    if payload.get("text"):
                        answer += str(payload["text"])
            stripped = line.replace(" ", "")
            if f'"id":{req["id"]}' in stripped:
                resp = json.loads(line)
                if resp.get("error"):
                    print(f"prompt.submit error: {resp['error']}")
                    return None
                continue  # {"status":"streaming"} ack - keep reading
            if '"message.complete"' in stripped:
                break
        if marker not in answer:
            print(f"bridge leg produced no marker: {answer[:200]!r}")
            return None
        return answer
    finally:
        proc.terminate()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--addr", default="127.0.0.1:50051")
    args = parser.parse_args()

    channel = grpc.insecure_channel(args.addr)
    stub = pbg.StewardServiceStub(channel)

    marker = f"CROSS-CLIENT-{int(time.time())}"

    # -- 1. Gateway/bridge path (desktop/web transport), retry on 429.
    answer = None
    for attempt in range(3):
        if attempt:
            print(f"retrying bridge leg (attempt {attempt + 1}) after backoff...")
            time.sleep(25 * attempt)
        answer = run_bridge_leg(args, marker)
        if answer is not None:
            break
    if answer is None:
        print("FAIL: bridge leg never produced the marker")
        return 1
    print("[1] gateway/bridge turn OK")

    # The steward session the bridge minted: the daemon session whose
    # transcript carries the marker (gateway session ids are bridge-local).
    listing = stub.ListChatSessions(pb.ListChatSessionsRequest(limit=10), timeout=30.0)
    steward_sid = ""
    for summary in listing.sessions:
        detail = stub.GetChatSession(pb.GetChatSessionRequest(session_id=summary.session_id), timeout=30.0)
        if marker in repr(detail):
            steward_sid = summary.session_id
            break
    if not steward_sid:
        print("FAIL: steward session carrying the marker not found in daemon store")
        return 1
    print(f"[1b] steward session resolved: {steward_sid}")

    # -- 2. CLI path: continue the SAME daemon session through gRPC directly.
    sid, answer2 = daemon_turn(stub, steward_sid, "Repeat the exact code word from my previous message, nothing else.")
    if sid != steward_sid:
        print(f"FAIL: session id drift: {steward_sid} -> {sid}")
        return 1
    if marker not in answer2:
        print(f"FAIL: follow-up lost history: {answer2[:300]}")
        return 1
    print(f"[2] CLI/gRPC same-session turn OK  session={sid}")

    # -- 3. Observation: daemon session list shows the session.
    sessions = stub.ListChatSessions(pb.ListChatSessionsRequest(limit=20), timeout=30.0)
    if not any(s.session_id == sid for s in sessions.sessions):
        print(f"FAIL: session {sid} not visible in daemon session list")
        return 1
    print(f"[3] daemon session list sees {sid}")

    print("CROSS-CLIENT E2E PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
