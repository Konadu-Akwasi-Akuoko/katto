#!/bin/bash
# Stop hook: forced visual QA of frontend design changes. If any src/**/*.tsx
# or *.css file was edited this session (marker armed by track-ui-edits.sh),
# block ending the turn and demand a Chrome-MCP QA pass over the changed
# surfaces at http://localhost:1420. If no dev server answers there, start a
# frontend-only one (`bun run dev`) and stop it again once the QA pass is
# done (the first Stop where the marker is no longer armed). A server the
# owner started themselves is never touched. Loop safety: the marker is
# consumed when the gate fires and only re-arms on further frontend edits,
# with a hard cap of 3 firings per session.
set -u

input=$(cat)

field() {
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$input" | jq -r "$1 // empty" 2>/dev/null
  else
    printf '%s' "$input" | python3 -c "import json,sys
print(json.load(sys.stdin).get(\"$2\") or '')" 2>/dev/null
  fi
}

session=$(field '.session_id' 'session_id')
marker="${TMPDIR:-/tmp}/katto-ui-qa-${session:-default}"
counter="${marker}.count"
server_pid_file="${marker}.devserver"

port_up() {
  curl -s -o /dev/null --max-time 2 http://localhost:1420
}

# Start the dev server as a detached process whose pid we can record exactly.
# The subshell `exec`s the server, so $! is the server itself rather than a
# short-lived wrapper; stdio is pinned to the log so the server can never hold
# this hook's stdout/stderr open and wedge the Stop pipeline.
start_our_server() {
  root="${CLAUDE_PROJECT_DIR:-$(pwd)}"
  bun_bin=$(command -v bun || echo /opt/homebrew/bin/bun)
  [ -x "$bun_bin" ] || return 1
  (
    cd "$root" || exit 1
    exec "$bun_bin" run dev
  ) < /dev/null > "${marker}.devserver.log" 2>&1 &
  srv=$!
  disown "$srv" 2>/dev/null || true
  printf '%s\n' "$srv" > "$server_pid_file"
  return 0
}

# Stop only the server this gate started (never the owner's own).
stop_our_server() {
  [ -f "$server_pid_file" ] || return 0
  pid=$(cat "$server_pid_file" 2>/dev/null)
  rm -f "$server_pid_file"
  # A malformed pid file must never turn into a kill of this hook or of init.
  case "$pid" in
    '' | *[!0-9]*) pid="" ;;
    "$$" | 1) pid="" ;;
  esac
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
    pkill -P "$pid" 2>/dev/null
    kill "$pid" 2>/dev/null
  fi
  sleep 0.3
  # bun run's child (vite) can outlive its parent; it is ours if the pid
  # file existed, so clear any survivor still holding the port.
  if port_up; then
    lsof -ti :1420 2>/dev/null | xargs kill 2>/dev/null
  fi
  return 0
}

# QA pass finished (no re-arm) or never armed: tidy up and let the turn end.
if [ ! -s "$marker" ]; then
  stop_our_server
  exit 0
fi

count=$(cat "$counter" 2>/dev/null || echo 0)
case "$count" in *[!0-9]*) count=0 ;; esac
if [ "$count" -ge 3 ]; then
  rm -f "$marker"
  stop_our_server
  exit 0
fi

started_note=""
if ! port_up; then
  start_our_server || exit 0
  up=""
  for _ in $(seq 1 40); do
    if port_up; then up=1; break; fi
    sleep 0.5
  done
  if [ -z "$up" ]; then
    stop_our_server
    echo "ui-qa-gate: could not start the dev server for visual QA (see ${marker}.devserver.log); skipping the QA pass" >&2
    exit 0
  fi
  started_note="
NOTE: this gate started a frontend-only dev server (bun run dev) for the QA
pass and will stop it automatically afterwards — do not kill it yourself,
and expect IPC-backed data to show empty/error states (design QA only)."
fi

changed=$(sort -u "$marker")
rm -f "$marker"
echo $((count + 1)) > "$counter"

cat >&2 <<EOF
ui-qa-gate: frontend design surfaces changed this session — run a visual QA
pass at http://localhost:1420 with the Chrome MCP tools
(mcp__claude-in-chrome__*, load via ToolSearch in ONE call) before ending
the turn.${started_note}

Changed files:
$changed

Required checks, on every surface those files render:
1. tabs_context_mcp first; reuse a localhost:1420 tab if one exists, else
   tabs_create_mcp. Navigate/refresh so the latest HMR state is loaded.
2. Screenshot each affected surface (computer tool) and actually look at it:
   nothing may overflow or spill past its container/card border; text must
   not clip or truncate unexpectedly.
3. Exercise every scrollable region you touched: scroll to the bottom,
   confirm content clips and scrolls inside the region (the app window
   itself must never scroll — that is a shell invariant).
4. resize_window to a narrow (~900px wide) and a wide (~1800px) layout and
   re-check the affected surfaces at both.
5. If the change touches tokens/theme CSS, check light AND dark theme.
6. read_console_messages and report any errors or warnings.
7. Audit the affected surfaces against the design rules in
   .claude/rules/design-system.md (with .claude/rules/frontend.md) — read
   them, then judge the screenshots against the "Banned — reads as
   AI-generated" list: mono-uppercase eyebrows/kickers, accent rails on
   cards, state encoded twice, mono as texture, interpunct filler,
   faux-metadata, bento grids, gradient heroes, glassmorphism,
   everything-centered, rounded-2xl-everywhere, and AI copy tells. Also
   confirm tokens-not-literals, serif/sans/mono roles, and 4px-grid
   spacing where visible. Any violation is a FAILURE: rework that UI now,
   then re-run this QA on the fix — do not merely note it.

Fix anything you find (this gate re-arms on further frontend edits and will
re-QA the fixes), and summarize what you checked and saw in your final
message. If the Chrome extension is not connected or a surface needs backend
state you cannot reach, run what you can and state exactly what was skipped
and why instead of retrying blindly.
EOF
exit 2
