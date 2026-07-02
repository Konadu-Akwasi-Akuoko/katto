---
paths:
  - "src/**/*.ts"
  - "src/**/*.tsx"
  - "src/**/*.css"
---

# Frontend rules (React 19 + TS + Vite + Tailwind v4)

## Structure and imports

- Feature folders: each surface lives in `src/features/<feature>/` (`components/`, `hooks/`, `store/`, `model/` as needed — create only what the feature uses). Composition happens in `src/app/`.
- Import flow is one-way: `shared (components/ui, lib, hooks, stores) → features → app`. Never import one feature from another; if two features need it, it moves to shared.
- No barrel files (`index.ts` re-exports). Import files directly via the `@/` alias.
- Pure logic (editor document ops, cut-list math, date math) lives in `features/<x>/model/` as pure functions with no React imports — that's what gets unit-tested.

## Naming

- Files kebab-case (`cut-list-panel.tsx`), components PascalCase (`CutListPanel`), hooks `use-<thing>.ts` exporting `useThing`. Tests colocated as `<name>.test.ts(x)`.

## IPC

- Never call `invoke("string")` from feature code. All IPC goes through `src/lib/ipc/<domain>.ts` typed wrappers over the generated tauri-specta bindings (`src/lib/ipc/bindings.gen.ts` — generated, never hand-edited).
- Rust errors arrive as tagged `{kind, message}`; mutations surface them via the TanStack Query error handlers mapped to toasts — not ad-hoc try/catch in components.

## State (the onion)

- Component-local → `useState`. Global UI (panel visibility, active tab, palette open) → Zustand stores in `src/stores/`, always selector syntax (`useUIStore(s => s.x)`), never whole-store destructuring. DB-backed data (projects, ideas, schedule, events, jobs) → TanStack Query over the typed IPC layer, query keys per domain, invalidate after mutations.
- The editor document is a dedicated Zustand store in `features/editor/store/` wrapped with zundo (`partialize` so only document state enters history; coalesce drags into one undo step). The live document never lives in TanStack Query; persistence is the explicit debounced auto-save mutation.

## Styling

- Tailwind v4 CSS-first: tokens in the `@theme` block of `src/styles/main.css`; semantic tokens (`--color-surface`) over raw palette values. No `@apply` outside rare base-layer cases. Variants via cva + `cn()`.
- Reach for an existing `src/components/ui/` primitive before writing a new interactive component; add new primitives via the shadcn CLI, then edit them freely — they're our code.
- Class order is machine-owned (prettier-plugin-tailwindcss); never hand-order.

## TypeScript

- `strict`, `noUncheckedIndexedAccess`, `verbatimModuleSyntax` are on and non-negotiable. No `any` — use `unknown` and narrow. `import type` for types.

## Tests

- Vitest + React Testing Library. Test pure `model/` functions and hooks first; component tests assert user-visible behavior via role/label queries. Do not test Tailwind classes, pixel styling, or DOM snapshots.
- Mock IPC with `@tauri-apps/api/mocks` (`mockIPC`); `clearMocks()` in `afterEach` is mandatory. Shared plumbing lives in `src/test/setup.ts`.
