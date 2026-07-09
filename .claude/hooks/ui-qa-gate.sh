#!/bin/bash
# Stop hook: forced visual QA of frontend design changes. If any src/**/*.tsx
# or *.css file was edited this session (marker armed by track-ui-edits.sh)
# and the dev server answers on http://localhost:1420, block ending the turn
# and demand a Chrome-MCP QA pass over the changed surfaces. Loop safety:
# the marker is consumed when the gate fires and only re-arms on further
# frontend edits (a QA pass that fixes things gets re-QA'd; a clean pass
# ends the turn), with a hard cap of 3 firings per session.
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

[ -s "$marker" ] || exit 0

count=$(cat "$counter" 2>/dev/null || echo 0)
case "$count" in *[!0-9]*) count=0 ;; esac
if [ "$count" -ge 3 ]; then
  rm -f "$marker"
  exit 0
fi

# QA needs the running app; without the dev server there is nothing to look
# at, so pass silently (the marker stays armed in case it comes up later).
if ! curl -s -o /dev/null --max-time 2 http://localhost:1420; then
  exit 0
fi

changed=$(sort -u "$marker")
rm -f "$marker"
echo $((count + 1)) > "$counter"

cat >&2 <<EOF
ui-qa-gate: frontend design surfaces changed this session and the dev server
is up at http://localhost:1420 — run a visual QA pass with the Chrome MCP
tools (mcp__claude-in-chrome__*, load via ToolSearch in ONE call) before
ending the turn.

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

Fix anything you find (this gate re-arms on further frontend edits and will
re-QA the fixes), and summarize what you checked and saw in your final
message. If the Chrome extension is not connected or a surface needs backend
state you cannot reach, run what you can and state exactly what was skipped
and why instead of retrying blindly.
EOF
exit 2
