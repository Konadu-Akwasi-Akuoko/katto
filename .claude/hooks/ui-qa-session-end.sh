#!/bin/bash
# SessionEnd hook: clean up after the UI QA gate — stop the dev server it
# started (never one the owner started) and drop the session's marker files.
set -u

input=$(cat)

if command -v jq >/dev/null 2>&1; then
  session=$(printf '%s' "$input" | jq -r '.session_id // empty' 2>/dev/null)
else
  session=$(printf '%s' "$input" | python3 -c 'import json,sys
print(json.load(sys.stdin).get("session_id") or "")' 2>/dev/null)
fi

marker="${TMPDIR:-/tmp}/katto-ui-qa-${session:-default}"
server_pid_file="${marker}.devserver"

if [ -f "$server_pid_file" ]; then
  pid=$(cat "$server_pid_file" 2>/dev/null)
  if [ -n "$pid" ]; then
    pkill -P "$pid" 2>/dev/null
    kill "$pid" 2>/dev/null
  fi
  sleep 0.3
  if curl -s -o /dev/null --max-time 2 http://localhost:1420; then
    lsof -ti :1420 2>/dev/null | xargs kill 2>/dev/null
  fi
fi

rm -f "$marker" "${marker}.count" "$server_pid_file" "${marker}.devserver.log"
exit 0
