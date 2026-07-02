# Phase 3 — SD Ingest

## Goal

Insert the camera card, get a notification, answer one question (which project?), and end with
verified, renamed footage in the right project folder and an ejected card. Dumb and
deterministic — **no AI in this path** (D10).

## Why this order

Ingest needs projects (Phase 2) to file into and the jobs framework (Phase 1) to report
through. It precedes the cut pipeline because the pipeline's input is footage that got here
via ingest. It is also the first real-hardware checkpoint.

## User stories

- I plug in the Sony card; the tray pulses and a notification offers "Import 14 clips".
- The import sheet asks only which project (defaulting to the one shooting nearest today);
  everything is pre-selected; I click Import.
- Copy progress shows in the window and the tray; when it finishes verified, katto offers to
  eject the card.
- My iPhone footage gets dragged into the same flat `footage/` folder by hand — same naming,
  no special path.
- The card is never written to. Ever.

## Scope with acceptance criteria

| Feature | Acceptance criteria |
|---|---|
| Volume watcher | Non-recursive `notify` watch on `/Volumes`; new mount debounced ~1 s until readable; unmount clears any pending offer; metadata via `diskutil info -plist` when needed |
| Card recognition | Volume is a camera card iff it contains `DCIM/` (generic + iPhone-style) or Sony `PRIVATE/M4ROOT/CLIP/`; recognizer returns the clip roots + card kind; non-camera volumes ignored silently |
| Clip enumeration | Walk clip roots for video extensions (`.mp4 .mov .mts .m4v` case-insensitive); per clip: name, size, duration + codec via `ffprobe -show_streams -show_format`; grouped by card substructure (e.g. `CLIP/`, `SUB/`); sidecar/thumbnail files (`.XML`, `.THM`) listed but deselected by default |
| Import sheet | One question — target project — defaulting to nearest `shoot_date`; "New project" inline (title → Phase-2 `create_project`); grouped file list all selected; total size + free-space check on the studio root |
| Copy job | Copy-only (source opened read-only, never modified/deleted); destination `footage/`; rename `YYYY-MM-DD_NNN.ext` where date = project shoot date if set for today ± else today, NNN = zero-padded 3-digit sequence continuing from the highest existing NNN for that date; collisions impossible by construction |
| Verification | Post-copy: file count matches selection; per-file size matches source byte count; mismatch → job `failed`, offending file quarantined as `<name>.partial`, card untouched |
| Progress + events | Jobs-framework job (`kind='ingest'`): per-file + overall progress over `Channel`; tray mirrors; `events` row `ingested {count, bytes, project}` on success |
| Eject | On success: "Eject card" button + tray action → `diskutil eject <device>`; failure to eject reported, never retried silently |
| iPhone / manual path | Dragging files onto the project detail's footage card runs the same rename+verify pipeline (no watcher involvement) |

## Backend (Rust)

New modules: `ingest.rs` + `ingest/` (`recognize.rs` — card detection, `enumerate.rs` — clip
listing, `naming.rs` — pure rename/sequence math, `copy.rs` — the job), `volumes.rs` (watcher),
`commands/ingest.rs`, `ffprobe.rs` (shared with Phase 4: spawn + JSON parse, pure parser
separated from the single spawn site).

Design rule (`.claude/rules/testing.md`): `recognize`, `enumerate` (given a walked tree),
`naming`, and the verification comparison are pure functions over in-memory tree
representations; only `copy.rs` and the `ffprobe` spawn touch the real filesystem.

## Frontend (React)

`src/features/ingest/`: import sheet (modal over any surface: project select, grouped clip
list with select-all per group, size summary, free-space warning), progress panel (reuses the
jobs progress component), success state with Eject button. Tray pulse = tray icon swap
(re-set template flag — Phase 1 gotcha) or badge.

## Wiring / IPC

| Command | Notes |
|---|---|
| `card_offer() -> Option<CardOffer>` | `CardOffer {volume, kind, groups: [{label, clips: [{path, name, size, duration_s?}]}]}` — current detected card if any |
| `start_ingest(volume, project_slug, selected_paths) -> job_id` | validates project + mount + free space, then spawns the copy job |
| `subscribe_job_progress(job_id, Channel<JobProgress>)` | Phase-1 mechanism; per-file messages |
| `eject_card(volume) -> ()` | diskutil |
| `import_files(project_slug, paths) -> job_id` | manual drag-in path |

Broadcast events: `card-detected {offer}`, `card-removed`; notification (Phase-2 notify) with
deep link `katto://ingest`.

## Data-model deltas

None. (Ingest state lives in `jobs` + `events`.)

## Error handling

- Free space insufficient → refuse to start with exact numbers.
- Copy error mid-job → job `failed` with the failed file + OS error; completed files stay
  (verified individually); `.partial` quarantine for the interrupted file; sheet offers retry
  of the remainder.
- Card yanked mid-copy → same failed path (source read error); nothing on the card was ever
  writable so nothing to corrupt.
- ffprobe failure on a clip → clip listed without duration, still importable (enumeration
  never blocks on metadata).
- Studio root unmounted at start → `StudioRootUnmounted` before any copy begins.

## Testing

- Pure: card recognition + grouping over fixture trees (`tests/fixtures/cards/` — Sony
  `PRIVATE/M4ROOT/CLIP`, generic `DCIM/100MEDIA`, iPhone `DCIM/100APPLE`, a non-camera
  volume); rename/sequence math (existing files, date rollover, NNN continuation, extension
  case); verification comparator (count/size mismatch cases).
- Integration (tempdir): copy job end-to-end with injected read failure; `.partial` quarantine.
- Manual hardware checkpoint: real Lexar V90 card from the ZV-E10 II — detect, import, verify,
  eject; iPhone folder drag-in.

## Out of scope

Checksummed offload, second-destination backup, hover-scrub thumbnails, AirDrop watcher (all
descoped by D10); transcode or proxy generation; any transcript/AI work (Phase 4).

## Exit criteria

Real Sony card inserted → notification → two clicks → verified renamed footage in the right
project, card ejected; `just check` green.
