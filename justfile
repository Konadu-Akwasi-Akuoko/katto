# katto development gate. CI mirrors `just check` 1:1 — if it passes here, it passes there.

default: check

# The full quality gate. Run before claiming any work done.
check: fmt-check clippy test tsc

fmt-check:
    cargo fmt --all -- --check

clippy:
    cargo clippy --all-targets -- -D warnings

test:
    cargo test

tsc:
    bunx tsc --noEmit

fmt:
    cargo fmt --all

# Phase 1 adds: `biome` (lint/format), `vitest` (frontend tests), and a real
# `bindings` recipe once the tauri-specta export exists. See prd/phase-1.md.
bindings:
    @echo "tauri-specta export lands in Phase 1 (prd/phase-1.md §Wiring)" && exit 1
