# design-sync notes — katto Design System

Repo-specific gotchas for future syncs. Read this before re-running.

## Shape & entry
- katto is a **Tauri app**, not a published component library. There is no library
  `dist` entry (`package.json` has no `main`/`module`/`exports`). We sync in the
  **package shape** using a hand-written barrel: `.design-sync/entry.ts` re-exports
  every `src/components/ui/*` file so the converter bundles all parts onto
  `window.Katto`. Pass it via `--entry ./.design-sync/entry.ts` — this also pins
  `PKG_DIR` to the repo root (walk-up finds `package.json` name "katto"), which is
  required: without `--entry`, PKG_DIR resolves to the nonexistent
  `node_modules/katto` and the build crashes in `exportedNames`.
- `componentSrcMap` lists the 16 top-level components explicitly (Card, Dialog, etc.
  export many compound parts — CardHeader, DialogContent — that ride in `window.Katto`
  but are NOT separate cards; compose them inside the parent's preview).
- Build command: `bun run build` (bun only, never npm/yarn).

## CSS / Tailwind 4
- Styling is Tailwind 4 (`@tailwindcss/vite`) with tokens in `src/styles/main.css`
  (`@theme inline`). The converter needs a **compiled** stylesheet, so `cssEntry`
  points at `.design-sync/compiled-tailwind.css` — a copy of the app's built CSS
  (`dist/assets/index-*.css`). **On re-sync: run `bun run build`, then re-copy**
  `cp dist/assets/*.css .design-sync/compiled-tailwind.css` before the converter,
  or the tokens/utilities go stale.
- Because the compiled CSS is Tailwind-purged to classes used *in the app*, preview
  layout glue should use **inline styles** (not arbitrary Tailwind utilities) so it
  doesn't depend on classes that may not be in the purged set. Component classes
  themselves are always present (the components are used in the app).

## Toaster — deferred to floor card (user-approved)
- **Toaster (sonner) is NOT statically previewable**: toasts are imperative/transient
  (`toast.*()` calls), portal to a fixed position the headless capture doesn't catch.
  Tried `useEffect` and `useLayoutEffect` + `expand`/`visibleToasts` — both render blank.
  No declarative sonner API exists, and a fake toast would violate ship-the-real-component.
  User chose (2026-07-03) to defer it to the **floor card**. Toaster ships fully functional
  in the bundle — the design agent can use `Toaster` + `toast()` and it renders in real
  interactive designs. A future re-sync can author it if a static approach is found.
- Consequence: Toaster's floor card renders blank (its root mounts a non-empty fixed
  container, so the typographic-block swap doesn't trigger) → a `[RENDER_BLANK]` warn on
  Toaster is EXPECTED, not new.

## Known render warns (triaged legitimate — not new issues)
- `[TOKENS_MISSING]` for `--color-border-2, --color-cut, --color-keep, --color-filler,
  --color-muted-2, --color-panel, --color-panel-2, --color-text`: these are
  app-surface tokens (cut/keep/filler are AI rough-cut decision colors; panel is
  layout). **None of the 16 UI primitives reference them** — verified by grep. They
  leak into the compiled CSS from app source but never affect these previews. Ignore.
- `[FONT_MISSING]` for SF Pro Text/Display, New York, Iowan Old Style: these are
  **macOS system fonts** the OS provides at runtime — nothing to ship. Suppressed via
  `cfg.runtimeFontPrefixes`. On a Mac the DS pane renders in SF Pro; elsewhere it
  falls back to system-ui. This is inherent to a macOS-only app using system fonts.

## Verification
- playwright 1.61.1 + chromium installed under `.ds-sync/node_modules` +
  `~/Library/Caches/ms-playwright` (macOS cache path, not `~/.cache`). Render check
  and capture pipeline run deterministically.

## Preview authoring learnings (proven across all 16)
- Import components from bare `'katto'` (rewritten to `window.Katto.*`). Editor tsc
  flags "Cannot find module 'katto'" — expected, harmless (esbuild + react-jsx compiles).
- Layout glue = inline styles only; design tokens (`--fg`, `--fg-muted`, `--border`,
  `--surface`) work inside inline `style`. Component-internal Tailwind classes are always
  present; arbitrary layout utilities may be purged-out.
- **Overlays/portals** (Dialog, Tooltip): render with `open` and set
  `cfg.overrides.<Name> = {cardMode:"single", primaryStory:"Open", viewport:"WxH"}` so the
  portal content renders inside the card.
- **Toaster** (sonner): imperative — fire `toast.*(..., {duration: Infinity})` in a
  `useEffect`, wrap `<Toaster position="top-center" expand />`; `toast` imports from `'sonner'`
  (bundled from source). cardMode single.
- **cmdk (Command)** does NOT filter on a static `defaultValue` in non-interactive capture —
  the `CommandEmpty` empty state is unreachable statically. Show populated groups, not filter-driven states.
- **ScrollArea** needs an explicit `height` on the `<ScrollArea>` element itself. The styled
  scrollbar thumb is hover-only (radix default) — absent in static screenshots; overflowing
  content + track is what's graded (acceptable).
- **Badge** has katto-specific job-state variants beyond shadcn (`running`/`done`/`failed`/
  `queued`) plus `ghost`/`link`. Its `.dot` child is styled by the component's own selector.
- **Tabs** `variant="line"` is a real `TabsList` cva variant — a distinct visual axis worth showing.

## Re-sync risks
- `cssEntry` is a build artifact copy — MUST re-copy after `bun run build` (above).
- The barrel `.design-sync/entry.ts` must be kept in sync when ui components are
  added/removed; likewise `componentSrcMap`.
- If the app adds real `--color-*` tokens that the ui primitives use, re-check the
  `[TOKENS_MISSING]` list — a new entry there could be a real missing token.
