# katto development gate. CI mirrors `just check` 1:1 — if it passes here, it passes there.

default: check

# The full quality gate. Run before claiming any work done.
check: fmt-check clippy test biome tsc vitest

fmt-check:
    cargo fmt --all -- --check

clippy:
    cargo clippy --all-targets -- -D warnings

test:
    cargo test

biome:
    bunx biome check .

tsc:
    bunx tsc --noEmit

vitest:
    bunx vitest run

fmt:
    cargo fmt --all

# Regenerate src/lib/ipc/bindings.gen.ts from the command registry.
bindings:
    cargo test -p katto --lib export_bindings

# Build the installable .app, signed with the stable dev identity when one
# exists (same keychain-ACL rationale as scripts/macos-dev-sign.sh); ad-hoc
# otherwise. Output: target/release/bundle/macos/katto.app
bundle:
    #!/bin/sh
    set -eu
    identity="${KATTO_DEV_SIGN_IDENTITY:-$(security find-identity -v -p codesigning 2>/dev/null | awk -F'"' 'NR == 1 && /"/ { print $2 }')}"
    if [ -n "$identity" ]; then
      export APPLE_SIGNING_IDENTITY="$identity"
    fi
    bun run tauri build --bundles app
