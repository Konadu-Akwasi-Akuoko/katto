---
name: add-feature-surface
description: Use when creating a new UI surface or feature area in the React app (a new sidebar tab, panel, window, or major view under src/features/).
---

# Add a feature surface

## Checklist (todo per item)

1. **Scaffold.** `src/features/<kebab-name>/` with only the subfolders the feature needs now (`components/`, `hooks/`, `store/`, `model/`). A feature can start as two files.
2. **Pure logic first.** Anything computable without React (list derivations, math, formatting decisions) goes in `model/` as pure functions — written test-first.
3. **Data.** DB-backed reads/writes via TanStack Query hooks in `hooks/`, calling `src/lib/ipc/<domain>.ts` wrappers (add commands via the add-tauri-command skill if the backend surface doesn't exist yet). Ephemeral UI state: feature-local Zustand store only if `useState` genuinely can't carry it.
4. **Compose.** Mount the surface from `src/app/` (sidebar entry / route / window). Features never import each other — shared pieces move to `src/components/` or `src/lib/`.
5. **Palette.** Register the surface's actions in the ⌘K command registry (every user-facing action in the app is reachable from the palette — design rule).
6. **UI primitives.** Build from `src/components/ui/`; add missing primitives via the shadcn CLI, then edit as our code.
7. **Tests.** Colocated: `model/` functions (unit), hooks with `mockIPC`, one behavior-level component test for the surface's core interaction.
8. **Gate.** `just check`.

## Common mistakes

- Importing another feature's components — move the shared piece to `src/components/` instead.
- Putting the surface's list state in Zustand when TanStack Query already caches it — Query is the source for DB-backed data.
- Skipping the palette registration — surfaces without palette commands are design-incomplete in this app.
