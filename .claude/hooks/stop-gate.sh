#!/bin/bash
# Stop hook: fast quality gate (~30s budget). Blocks ending the turn with a
# broken tree. Runs only the checks whose source files actually changed; the
# full gate (clippy, tests) lives in `just check` / CI.
set -u

input=$(cat)

# Avoid re-block loops: if we're already continuing from a stop hook, pass.
if command -v jq >/dev/null 2>&1; then
  active=$(printf '%s' "$input" | jq -r '.stop_hook_active // false' 2>/dev/null)
else
  active=$(printf '%s' "$input" | python3 -c 'import json,sys; print(str(json.load(sys.stdin).get("stop_hook_active", False)).lower())' 2>/dev/null)
fi
[ "$active" = "true" ] && exit 0

root="${CLAUDE_PROJECT_DIR:-$(pwd)}"
cd "$root" || exit 0

changed=$(git status --porcelain 2>/dev/null | awk '{print $NF}')
[ -n "$changed" ] || exit 0

fail() {
  echo "stop-gate: $1" >&2
  exit 2
}

if printf '%s\n' "$changed" | grep -q '\.rs$'; then
  if command -v cargo >/dev/null 2>&1; then
    out=$(cargo fmt --all -- --check 2>&1) || fail "cargo fmt --check failed:
$out
Run 'cargo fmt --all' before finishing."
  fi
fi

if printf '%s\n' "$changed" | grep -Eq '\.(ts|tsx)$'; then
  biome="$root/node_modules/.bin/biome"
  if [ -x "$biome" ]; then
    out=$("$biome" check src 2>&1) || fail "biome check failed:
$out"
  fi
  tsc="$root/node_modules/.bin/tsc"
  if [ -x "$tsc" ]; then
    out=$("$tsc" --noEmit 2>&1) || fail "tsc --noEmit failed:
$out"
  fi
fi

exit 0
