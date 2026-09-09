#!/usr/bin/env bash
# Unified e2e suite — one command proving the four client surfaces against
# the live Steward daemon:
#   1. OMP CLI    (pty)          — e2e/omp_pty_e2e.sh
#   2. Desktop    (Electron)     — vendor/hermes/apps/desktop steward-daemon spec
#   3. Web        (browser)      — vendor/hermes/web-e2e steward-web spec
#   4. Gateway bridge + gRPC     — e2e/cross_client_e2e.py (same-session)
#
# Requires: daemon running (hub steward-daemon-omp), hermes-dashboard-web
# server on :8093 (for the web spec), npm deps installed, bun 1.4.x at
# %LOCALAPPDATA%/bun-bin.
set -u
cd "$(dirname "$0")/.."
PASS=0; FAIL=0

run() {
  local name="$1"; shift
  echo "=== $name ==="
  if "$@" > /tmp/steward-e2e-$name.log 2>&1; then
    echo "--- PASS: $name"; PASS=$((PASS+1))
  else
    echo "--- FAIL: $name (tail:)"; tail -20 /tmp/steward-e2e-$name.log; FAIL=$((FAIL+1))
  fi
}

run cli bash e2e/omp_pty_e2e.sh
run cross-client python e2e/cross_client_e2e.py
run desktop bash -c "cd vendor/hermes/apps/desktop && npx playwright test e2e/steward-daemon.spec.ts --reporter=line"
run web bash -c "cd vendor/hermes && npx playwright test web-e2e --config=web-e2e/playwright.config.ts --reporter=line"

echo "=== unified suite: $PASS passed, $FAIL failed ==="
[ "$FAIL" -eq 0 ]
