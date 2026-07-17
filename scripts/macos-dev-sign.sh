#!/bin/sh
# Cargo runner (see .cargo/config.toml): signs the freshly built binary with a
# stable identity before executing it, so macOS keychain "Always Allow" grants
# survive rebuilds. Ad-hoc (linker) signatures change on every build, and the
# keychain binds its ACLs to the signature — a stable identity is the fix
# (prd/phase-1.md "Keychain gotcha"). With no identity available, runs the
# binary unsigned exactly as before.
set -eu

binary="$1"
shift

identity="${KATTO_DEV_SIGN_IDENTITY:-}"
if [ -z "$identity" ]; then
  identity="$(security find-identity -v -p codesigning 2>/dev/null \
    | awk -F'"' 'NR == 1 && /"/ { print $2 }')"
fi

if [ -n "$identity" ]; then
  codesign --force --sign "$identity" \
    --identifier com.akwasikonaduakuoko.katto "$binary" \
    || echo "macos-dev-sign: codesign failed; running unsigned" >&2
fi

exec "$binary" "$@"
