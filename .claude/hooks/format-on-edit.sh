#!/bin/bash
# PostToolUse hook (Edit|Write): format the edited file by extension.
# Probes tool availability and no-ops with a note when a formatter isn't
# installed yet (Biome/prettier arrive with the Phase 1 frontend toolchain).
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
[ -n "$file" ] && [ -f "$file" ] || exit 0

root="${CLAUDE_PROJECT_DIR:-$(pwd)}"

case "$file" in
  *.rs)
    if command -v rustfmt >/dev/null 2>&1; then
      rustfmt --edition 2024 "$file" 2>/dev/null || true
    else
      echo "format-on-edit: rustfmt not on PATH; skipped $file"
    fi
    ;;
  *.ts | *.tsx | *.json | *.css)
    biome="$root/node_modules/.bin/biome"
    if [ -x "$biome" ]; then
      "$biome" check --write "$file" >/dev/null 2>&1 || true
    else
      echo "format-on-edit: biome not installed yet (Phase 1); skipped $file"
    fi
    # Tailwind class order is owned by prettier-plugin-tailwindcss (Biome's
    # sorter can't handle custom variants); run only when the plugin exists.
    case "$file" in
      *.tsx | *.css)
        prettier="$root/node_modules/.bin/prettier"
        if [ -x "$prettier" ] && [ -d "$root/node_modules/prettier-plugin-tailwindcss" ]; then
          "$prettier" --write "$file" >/dev/null 2>&1 || true
        fi
        ;;
    esac
    ;;
esac

exit 0
