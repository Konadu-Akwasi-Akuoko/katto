# katto development gate. CI mirrors `just check` 1:1 — if it passes here, it passes there.

default: check

# The full quality gate. Run before claiming any work done.
check: fmt-check clippy test tsc vitest

fmt-check:
    cargo fmt --all -- --check

clippy:
    cargo clippy --all-targets -- -D warnings

test:
    cargo test

tsc:
    bunx tsc --noEmit

vitest:
    bunx vitest run

fmt:
    cargo fmt --all

# Regenerate src/lib/ipc/bindings.gen.ts from the command registry.
bindings:
    cargo test -p katto --lib export_bindings
