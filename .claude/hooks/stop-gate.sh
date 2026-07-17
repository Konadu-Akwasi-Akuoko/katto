#!/bin/bash
# Stop hook: quality gate. Blocks ending the turn with a broken tree.
#
# Mirrors `just check` (fmt-check, clippy, cargo test, biome, tsc, vitest) —
# the same checks CI runs — but only the halves whose source actually changed:
# touch no Rust and you pay nothing for Rust. Touch neither language and this
# exits before doing any work.
#
# Cost, warm: clippy ~0.3s, cargo test ~10s, tsc+biome ~2s, vitest ~9s. After a
# real edit the compile is not cached and Rust costs more — that is the point.
#
# Ordering matches `just check`: clippy before cargo test, so a crate that does
# not build says so instead of failing an assertion 10 seconds later. Clippy
# compiles, which is why there is no separate `cargo check`.
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

# -z gives NUL-terminated, unquoted paths, so names with spaces survive; the
# rename form "R old\0new\0" leaves both halves in the list, which is what we
# want — either side changing is a reason to check that language.
changed=$(git status --porcelain -z 2>/dev/null | tr '\0' '\n')
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
    out=$(cargo clippy --all-targets -- -D warnings 2>&1) || fail "cargo clippy failed:
$out"
    out=$(cargo test 2>&1) || fail "cargo test failed:
$out"
  fi
fi

if printf '%s\n' "$changed" | grep -Eq '\.(ts|tsx)$'; then
  biome="$root/node_modules/.bin/biome"
  if [ -x "$biome" ]; then
    out=$("$biome" check . 2>&1) || fail "biome check failed:
$out"
  fi
  tsc="$root/node_modules/.bin/tsc"
  if [ -x "$tsc" ]; then
    out=$("$tsc" --noEmit 2>&1) || fail "tsc --noEmit failed:
$out"
  fi
  vitest="$root/node_modules/.bin/vitest"
  if [ -x "$vitest" ]; then
    out=$("$vitest" run 2>&1) || fail "vitest failed:
$out"
  fi
fi

exit 0
