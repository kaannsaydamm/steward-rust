#!/bin/bash
cd "$(dirname "$0")"
powershell -Command "Get-Process steward-daemon -ErrorAction SilentlyContinue | Stop-Process -Force" 2>/dev/null
sleep 1
./target/debug/steward-daemon.exe --port 18960 --web-port 18961 > /tmp/d.log 2>&1 &
D=$!
sleep 14
echo "listener: $(netstat -ano | grep ':18960.*LISTENING' | head -1)"
echo "--- bad json:"
curl -s -m 15 -X POST "http://127.0.0.1:18960/omp/v1/chat/completions" -H "Content-Type: application/json" --data-binary '{bad'
echo " (exit=$?)"
echo "--- good json (live, up to 170s):"
curl -s -m 170 -X POST "http://127.0.0.1:18960/omp/v1/chat/completions" -H "Content-Type: application/json" --data-binary '{"model":"m","messages":[{"role":"user","content":"Reply with exactly: OMP-BRIDGE-OK"}]}'
echo " (exit=$?)"
kill $D 2>/dev/null
powershell -Command "Get-Process steward-daemon -ErrorAction SilentlyContinue | Stop-Process -Force" 2>/dev/null
echo DONE
