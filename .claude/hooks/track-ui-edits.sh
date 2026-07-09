#!/bin/bash
# PostToolUse hook (Edit|Write): arm the UI QA gate whenever a frontend
# design surface changes. Records edited src/**/*.tsx and src/**/*.css paths
# (tests excluded) in a per-session marker consumed by ui-qa-gate.sh.
set -u

input=$(cat)

field() {
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$input" | jq -r "$1 // empty" 2>/dev/null
  else
    printf '%s' "$input" | python3 -c "import json,sys
d=json.load(sys.stdin)
for k in \"$2\".split('.'):
    d=d.get(k,{}) if isinstance(d,dict) else {}
print(d if isinstance(d,str) else '')" 2>/dev/null
  fi
}

file=$(field '.tool_input.file_path' 'tool_input.file_path')
[ -n "$file" ] || exit 0

root="${CLAUDE_PROJECT_DIR:-$(pwd)}"
case "$file" in
  "$root"/src/*.tsx | "$root"/src/*.css) ;;
  *) exit 0 ;;
esac
case "$file" in
  *.test.tsx | "$root"/src/test/*) exit 0 ;;
esac

session=$(field '.session_id' 'session_id')
marker="${TMPDIR:-/tmp}/katto-ui-qa-${session:-default}"
printf '%s\n' "${file#"$root"/}" >> "$marker"

exit 0
