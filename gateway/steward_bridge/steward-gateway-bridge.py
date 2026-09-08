#!/usr/bin/env python
"""Steward launcher for the vendored Hermes gateway.

Boots the upstream ``tui_gateway`` JSON-RPC server with ``run_agent``
resolved to the Steward adapter (gateway/steward_bridge/run_agent.py),
so every prompt/tool/approval the Desktop or Web sends flows to the
Steward daemon instead of the Hermes agent runtime.

Usage:
  python steward-gateway-bridge.py [--addr 127.0.0.1:50051] [--port N]

Gateway transport stays upstream (stdio + WS). Contract unchanged.

ADAPTER code — docs/OSS_PORT_MAP.md H4.
"""

from __future__ import annotations

import argparse
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
GATEWAY = os.path.join(REPO, "vendor", "hermes", "tui_gateway")
BRIDGE = os.path.join(REPO, "gateway", "steward_bridge")

# import order matters: the bridge dir must win `import run_agent`.
sys.path.insert(0, BRIDGE)
sys.path.insert(1, GATEWAY)
# hermes-agent repo root supplies agent/, tools/, hermes_cli/, hermes_constants/…
HERMES_ROOT = os.path.join(REPO, "vendor", "hermes")
if os.path.isdir(HERMES_ROOT):
    sys.path.append(HERMES_ROOT)

os.environ.setdefault("STEWARD_DAEMON_ADDR", "127.0.0.1:50051")


def main() -> int:
    parser = argparse.ArgumentParser(description="Steward gateway bridge")
    parser.add_argument("--addr", default=os.environ["STEWARD_DAEMON_ADDR"])
    parser.add_argument("--port", type=int, default=0, help="JSON-RPC WS port (0 = upstream default)")
    args, rest = parser.parse_known_args()
    os.environ["STEWARD_DAEMON_ADDR"] = args.addr

    # smoke the daemon before booting the server: fail loud, fail early
    sys.path.insert(0, BRIDGE)
    from steward_agent import ping  # noqa: E402

    # Import run_agent EARLY: importing it registers the steward.profiles.*
    # JSON-RPC methods into the gateway method table (adapter extension).
    import run_agent  # noqa: E402,F401

    status = ping(args.addr)
    print(f"[steward-bridge] daemon ping: {status} @ {args.addr}", flush=True)

    if args.port:
        os.environ.setdefault("HERMES_GATEWAY_PORT", str(args.port))

    # hand control to the upstream entry point, unchanged
    sys.argv = [sys.argv[0], *rest]
    from tui_gateway.entry import main as gateway_main  # noqa: E402

    return gateway_main() or 0


if __name__ == "__main__":
    raise SystemExit(main())
