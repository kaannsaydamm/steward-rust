#!/usr/bin/env bash
# OMP CLI pty E2E: the vendored OMP CLI, configured with a steward provider
# (models.yml baseUrl -> daemon /omp/v1 bridge), answers a prompt through the
# Steward daemon. Requires: daemon running (hub steward-daemon-omp), bun,
# vendored omp tree.
#
# Usage: bash e2e/omp_pty_e2e.sh
set -u
cd "$(dirname "$0")/.."

python.exe -c "import re; wp = open(r'C:/Users/kaann/.steward/wire-port', encoding='utf-8').read().strip(); mpath = r'C:/Users/kaann/.omp/agent/models.yml'; text = open(mpath, encoding='utf-8').read(); text = re.sub(r'(steward:\n    baseUrl: http://127\.0\.0\.1:)\d+', r'\g<1>' + wp, text); open(mpath, 'w', encoding='utf-8').write(text); print('provider rewired to wire port', wp)"

cd vendor/omp
BUN="C:/Users/kaann/AppData/Local/bun-bin/bun.exe"  # modern bun 1.4.x; PATH bun (npm shim 1.2.21) is too old and ~/.bun is exec-blocked
OUTPUT=$("$BUN" packages/coding-agent/src/cli.ts --model steward/glm-5.3-flash -p "Reply with exactly: OMP-E2E-OK" 2>&1)
EXIT=$?
echo "$OUTPUT" | tail -3
if [ "$EXIT" -eq 0 ] && echo "$OUTPUT" | grep -q "OMP-E2E-OK"; then
  echo "OMP PTY E2E PASS"
  exit 0
fi
echo "OMP PTY E2E FAILED (exit=$EXIT)"
exit 1
