# Phase 2 — Projects & Planner

## Goal

The planning spine: ideas captured from anywhere, triaged in a Backlog, promoted into real
project folders in the studio root in one motion, visualized on a Board and Calendar, surfaced in the
tray — with the filesystem, not the DB, as the source of truth.

## Why this order

Projects are the noun every later phase acts on (ingest files into them, the cut editor lives
inside them, the dock works in their folders). The promote flow fixes the slug contract early
— everything downstream joins on folder name.

## Progress

Living checklist — the durable record of what's landed vs. outstanding on `feat/phase-2-projects-planner`.

- [x] CI rust job moved to `macos-latest` (Phase 2 lands objc2/macOS-only deps)
- [x] Slug contract — `kebab_slug` + `project_slug` ported byte-compatible from the hyper-frames studio source (collision ladder table-tested)
- [x] `project.json` manifest — schema-validated read/write, atomic `.tmp` → rename round-trip
- [x] D6 folder anatomy skeleton + guarded rollback removal (never recreates, never deletes on invalid manifest)
- [x] Migration 002 (`projects.last_touched_at`) + `db/projects.rs` repository
- [x] `db/ideas.rs` + `db/schedule.rs` repositories
- [x] Reconcile — folder scan + pure diff + launch hook + `rescan_projects` (folders are truth)
- [x] Projects commands — CRUD + freshness grid + `projects-changed` broadcast
- [x] Ideas commands — CRUD + one-transaction promote with rollback and folder cleanup
- [x] Schedule commands + tray current-project and next-shoot lines
- [x] Planner shell + Backlog triage surface
- [x] Board — kanban with drag-to-status (dnd-kit)
- [x] Calendar — month/week grids with shoot + publish chips + week-ahead model
- [x] Projects list + detail surfaces (freshness grid, reveal-in-Finder)
- [x] Quick capture — global hotkey + capture window into the backlog
- [x] Notifications + `katto://` deep links (dev degrades to tray)
- [x] Palette commands — new idea, promote idea, new project, go to project, open studio root
- [x] Capture window opens on the active Space (`visible_on_all_workspaces`) with a soft `--r-lg` frame; tray gained a "Quick capture" item
- [x] Settings rebind for `capture_shortcut` — `set_capture_shortcut` validates and re-registers before persisting; Settings → General has the recorder row with a reset-to-default affordance
- [ ] Capture window over a **fullscreen** Space — still broken ([tauri#11488](https://github.com/tauri-apps/tauri/issues/11488)): OR-ing `CanJoinAllSpaces | FullScreenAuxiliary` into `collectionBehavior` via `objc2-app-kit` landed but the manual pass still shows the window opening on katto's own Space, not the fullscreen one. Needs a thorough investigation — parked in `TODO.md` (leads: non-activating `NSPanel`, activation policy, window level)
- [x] Exit criteria: owner's manual pass (hotkey → promote → folder + Board card; Calendar shoot date; tray shoot line; Finder-delete → rescan; SSD unplug/replug) — signed off 2026-07-17; the fullscreen-Space capture gap is tracked separately above and in `TODO.md`

## User stories

- Watching a video at lunch, I hit the global hotkey, type an idea, hit return — it's in the
  backlog without katto's window ever opening.
- In the Backlog I discard weak ideas and promote a strong one; a project folder appears in the
  studio root and a card appears on the Board, in one motion.
- I set the shoot day and publish day; the Calendar shows my week and the tray says
  "shooting Thursday: NVMe deep dive".
- I delete a project folder in Finder; the card is gone next launch. katto never "helpfully"
  recreates it.

## Scope with acceptance criteria

| Feature | Acceptance criteria |
|---|---|
| Folder anatomy | `create_project` builds `<StudioRoot>/Projects/<slug>/` with `project.json` + `footage/ audio/ assets/envato assets/vfx assets/graphics assets/music assets/sfx thumbnails/ timelines/ exports/` (D6); manifest schema-validated on read and write |
| Slug contract | `slug = kebab(title) + "-" + YYYY-MM-DD`; collision dedupe inserts `-2-`, `-3-` before the date, byte-compatible with hyper-frames `tools/studio/server/lib/util.ts` `kebabSlug` + `routes/ideas.ts` dedupe; folder name always equals the stored slug / `promoted_slug` exactly |
| Manifest | `project.json = {schema_version, slug, title, status, target_nle, shoot_date?, publish_date?, created_at, links: {}}`; atomic writes (`.tmp`→rename) |
| Scan/reconcile | On launch and on demand: scan `<StudioRoot>/Projects/*`; folder with valid manifest → upsert DB row; DB row with no folder → delete row (+ `events` row `project-vanished`); invalid manifest → surfaced in dashboard, row untouched |
| Ideas CRUD | create (manual), edit title/kind/notes, discard (status=`discarded`, kept as audit), list by status; **no score/rank fields anywhere** (D7) |
| Promote | One transaction: idea → `status='promoted'` + `promoted_slug`; project row inserted; folder skeleton created; failure anywhere rolls back DB and removes any partial folder; `events` row |
| Schedule | shoot/publish rows per project; week-ahead query for tray + dashboard |
| Quick capture | Global hotkey (default ⌥⌘K, rebindable in Settings) opens a small always-on-top borderless window (title input, optional note, kind picker) from any app; Enter saves `ideas` row (type=`manual`, status=`backlog`) + closes; Esc cancels; works while main window hidden, and opens on the **active** Space, not the app's own |
| Notifications + deep links | `notify(title, body, url)` via `objc2-user-notifications` (bundled identified app required — official plugin has no desktop click handlers); clicking opens/focuses katto at `katto://ideas` or `katto://project/<slug>`; in dev (unsigned), notifications degrade to tray attention + events row |
| Planner Board | Kanban columns over project status (`idea → shooting → editing → published` v1 vocabulary); drag card between columns persists status; card shows title, next date chip, latest-artifact hint |
| Planner Calendar | Month + week views; shoot chips and publish chips; click → project detail |
| Planner Backlog | Ideas list (status=`backlog`) with promote/discard/edit inline; promoted idea animates out toward the Board |
| Projects list + detail | List with status/date; detail shows manifest fields, per-subfolder freshness (latest mtime + count), "Reveal in Finder" per folder (opener plugin) |
| Tray | Current project line = most recently touched project; next-shoot line from schedule |
| Palette | new idea, promote idea…, new project, go to project…, open studio root |

## Backend (Rust)

New modules: `projects.rs`/`projects/` (anatomy, manifest, slug, reconcile), `commands/projects.rs`,
`commands/ideas.rs`, `commands/schedule.rs`, `capture.rs` (hotkey + capture window),
`notify.rs` (objc2-user-notifications wrapper + deep-link routing), `db/{projects,ideas,schedule}.rs`.

Crates added: `tauri-plugin-global-shortcut 2.3` (Carbon hotkey — no Accessibility permission;
handle `ShortcutState::Pressed` only), `tauri-plugin-opener` (already present),
`tauri-plugin-deep-link` (register `katto://`), `objc2-user-notifications` + `objc2` (CI note:
this lands the first macOS-only dep — switch CI runner to `macos-latest` now),
`notify 8.x` + `notify-debouncer-full` (studio-root watch groundwork; full use in Phase 3).

Slug port: implement `kebab_slug` + dedupe as pure functions; port semantics from
`hyper-frames/tools/studio/server/lib/util.ts` and `server/routes/ideas.ts` (read them, match
behavior test-for-test).

## Frontend (React)

```
src/features/
  planner/
    board/        # columns, dnd (dnd-kit), status mutation
    calendar/     # month/week grids, chips (pure date math in model/)
    backlog/      # ideas table, promote/discard actions
    model/        # week-ahead derivation, column grouping (pure, tested)
  projects/
    list/, detail/  # detail: manifest card, folder freshness grid, reveal buttons
  capture/        # quick-capture window surface (separate WebviewWindow, minimal shell)
```

Quick-capture window is its own Tauri `WebviewWindow` (`capture`, ~420×160, always-on-top,
skip-taskbar, transparent frame) rendering only the capture form route.

## Wiring / IPC

| Command | Notes |
|---|---|
| `list_projects() -> Vec<Project>` / `get_project(slug)` | |
| `create_project(title, shoot_date?) -> Project` | builds folder + row |
| `set_project_status(slug, status)` / `set_project_dates(slug, shoot?, publish?)` | board drag + detail edits |
| `rescan_projects() -> ReconcileReport` | `{added, removed, invalid_manifests}` |
| `reveal_project_folder(slug, subfolder?)` | opener plugin |
| `list_ideas(status) -> Vec<Idea>` / `create_idea(input)` / `update_idea(id, patch)` / `discard_idea(id)` | |
| `promote_idea(id) -> {slug}` | the one-transaction promote |
| `list_schedule(range) -> Vec<ScheduleEntry>` / `upsert_schedule_entry` / `delete_schedule_entry` | |
| `capture_submit(title, note?, kind?)` | from the capture window |

Broadcast events: `projects-changed`, `ideas-changed`, `schedule-changed` (Query invalidation);
`deep-link {route}` → frontend router.

## Data-model deltas

Schema from Phase 1 already carries `projects`, `ideas`, `schedule`. Migration 002 only if
reconcile needs it: `projects.last_touched_at TEXT` (tray "current project" heuristic).

## Error handling

- Promote partial failure → rollback + folder cleanup + `Error::PromoteFailed {stage}`;
  idea stays `backlog`.
- Studio root unmounted → all folder-touching commands return `StudioRootUnmounted`; Planner
  stays readable (DB), creation/promotion disabled with the banner explaining why.
- Invalid `project.json` → project listed with an "invalid manifest" badge + raw error in
  detail; reconcile never deletes a folder's row for being invalid (only for being absent).
- Hotkey registration conflict → settings shows "in use by another app", offers rebind.
- Notification permission denied → recorded once in events; deep-link path still works from
  the tray.

## Testing

- Pure: `kebab_slug` + dedupe (table-driven vs cases derived from the hyper-frames source,
  including collision ladders), reconcile diffing (added/removed/invalid fixtures), manifest
  round-trip, week-ahead date math (TS, `planner/model/`).
- DB: ideas CRUD + promote transaction (rollback on injected failure) on `:memory:`.
- Frontend: backlog promote/discard flows with `mockIPC`; board column grouping.
- Manual: real hotkey from another app; Finder-delete → rescan removes card; SSD unplug/replug.

## Out of scope

Ingest (Phase 3), any AI (4/6), notifications-with-actions beyond open-at-route, board stages
beyond the v1 vocabulary (the 12-stage hyper-frames ladder stays in tools/studio; katto's board
tracks katto's pipeline).

## Carried forward

- **A refresh control that reconciles folders → DB without relaunching.** `rescan_projects`
  (`src-tauri/src/commands/projects.rs:39`) and its typed wrapper already exist and work — only
  the UI trigger is missing, so today reconcile runs on launch and nowhere else. Two strings
  already promise the control: `board-view.tsx:156` ("then rescan from Projects") and the delete
  dialog's "katto picks it up on the next rescan" (`:205`). Confirmed by hand 2026-07-17: a
  trashed project Put Back from the Bin only reappears after a restart. Wire it, and the copy
  stops lying.

## Exit criteria

Idea captured via the global hotkey from another app → promoted → real folder in the studio root +
card on the Board; Calendar shows its shoot date; tray shows "shooting <day>: <title>";
`just check` green.
