#!/bin/bash
# PreToolUse hook (Edit|Write): block edits to generated or derived files.
# Exit 2 = block the tool call; stderr is fed back to Claude.
set -u

input=$(cat)

extract_path() {
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$input" | jq -r '.tool_input.file_path // empty' 2>/dev/null
  else
    printf '%s' "$input" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("tool_input",{}).get("file_path",""))' 2>/dev/null
  fi
}

file=$(extract_path)
[ -n "$file" ] || exit 0

block() {
  echo "Blocked: $file is generated/derived — $1" >&2
  exit 2
}

case "$file" in
  */src-tauri/gen/* | src-tauri/gen/*)
    block "Tauri regenerates it; change tauri.conf.json or capabilities instead." ;;
  *bindings.gen.ts)
    block "regenerate via the specta export (just bindings) instead of editing." ;;
  *.snap.new)
    block "run 'cargo insta review' to accept/reject snapshots." ;;
  */Cargo.lock | Cargo.lock | */bun.lock | bun.lock)
    block "lockfiles are updated by cargo/bun commands, never edited." ;;
esac

exit 0
