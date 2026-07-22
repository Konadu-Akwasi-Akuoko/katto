# PRD index

## Doc map

| Doc | Contents |
|-----|----------|
| [README.md](README.md) | Vision · architecture · locked decisions log (D1–D20) · supersedes note · source-availability note |
| [phase-1.md](phase-1.md) | Shell & First Light — tray, window, palette, onboarding, SQLite, events, jobs, settings |
| [phase-2.md](phase-2.md) | Projects & Planner — folder anatomy, reconcile, Board/Calendar/Backlog, quick capture, promote |
| [phase-3.md](phase-3.md) | SD Ingest — volume watch, card recognition, copy/rename/verify, eject |
| [phase-4.md](phase-4.md) | Cut Pipeline — Rational, validators, import, transcribe, CutPlanner, CLI, pipeline UI, read-only review |
| [phase-5.md](phase-5.md) | Cut Editor & Export — 3-pane editing, undo, FCPXML 1.11 + rescue track, MP4, SRT/VTT |
| [phase-6.md](phase-6.md) | Claude Dock & Automations — session pool, terminal panel, scheduler, nightly curation, VFX cockpit |
| [phase-7.md](phase-7.md) | Browser, Thumbnails, Resolve, studio.db import, polish, packaging |

Every phase is a vertical slice (Rust backend + React UI wired via IPC) with its own exit
criterion. Implement a phase in its own session from its PRD plus the committed guidance layer
(`CLAUDE.md`, `.claude/rules/`, `.claude/skills/`).

## Feature → phase matrix

Every feature from the approved design (§3–§9), mapped. Nothing dropped.

| Design area | Feature | Phase |
|---|---|---|
| §3 Project model | Studio root (any dir; external SSD recommended), picker at onboarding | 1 |
| §3 | "Drive disconnected" state + reconnect hint | 1 |
| §3 | Project folder anatomy + `project.json` manifest | 2 |
| §3 | Folder-is-truth scan/reconcile on launch | 2 |
| §3 | `.kruproj` bundle inside project folder; loose-bundle open | 4 |
| §4 Data model | SQLite schema (`ideas`, `projects`, `schedule`, `events` + `jobs`, `scheduled_jobs`) | 1 |
| §4 | Events log (append-only, powers dashboard + diagnostics) | 1 |
| §4 | Ideas backlog CRUD (no scoring) | 2 |
| §4 | Global quick-capture hotkey | 2 |
| §4 | Planner Board (Kanban) / Calendar / Backlog views | 2 |
| §4 | Promote = folder + card in one motion (slug contract) | 2 |
| §4 | Schedule (shoot/publish days), week-ahead + tray surfacing | 2 |
| §4 | Anacron-style scheduler with catch-up | 6 |
| §4 | Nightly idea curation via visible Claude session + notification deep-link | 6 |
| §4 | One-time `studio.db` idea import | 7 |
| §5 Ingest | Volume watcher + camera-card recognition (`DCIM/`, `PRIVATE/M4ROOT/`) | 3 |
| §5 | Import sheet (project question, defaults, inline new-project) | 3 |
| §5 | Copy-only + `YYYY-MM-DD_NNN.ext` rename + count/size verification | 3 |
| §5 | Progress in window + tray; eject offer; events row | 3 |
| §6 Cut editor | Import → audio extraction (`ffprobe`/`ffmpeg`) | 4 |
| §6 | ElevenLabs Scribe v2 transcription | 4 |
| §6 | Claude cut-plan (`CutPlanner`: subprocess + HTTP modes, BYOK, keychain) | 4 |
| §6 | cuts.json validation (invariants + retry-once) | 4 |
| §6 | Pipeline step indicator with `Channel<T>` streaming; incremental cut arrival | 4 |
| §6 | Transcript pane (token spans, click-to-seek, cut/discretionary/flag affordances) | 4 (read-only) / 5 (editing) |
| §6 | Video pane (kept-only playback, show-original, J/K/L transport) | 5 |
| §6 | Timeline pane (canvas, thumbnails, region drag, token snap) | 5 |
| §6 | Waveform (wavesurfer v7) | 5 |
| §6 | Undo/redo persisted across sessions; 200 ms debounced auto-save | 5 |
| §6/§7 | FCPXML 1.11 export **with rescue track**; versioned `timelines/`; "Open in Final Cut" | 5 |
| §7 | NLE target (`default_nle`): seeded at first export, sticky last-used default | 5 |
| §7 | MP4 render (deterministic filtergraph) + SRT/VTT captions | 5 |
| §7 | CLI (`katto cut/import/transcribe/plan/render/export/auth status`) | 4–5 |
| §7 | Resolve Studio scripting-API import ("Open in Resolve") | 7 |
| §7 Dock | Session-pool manager (PTY, one tab per task, scale on demand) | 6 |
| §7 | Dock icon states; slide-over xterm.js panel; push API; idle reaping | 6 |
| §7 | Cut planning re-routed through a dock session | 6 |
| §7 | VFX cockpit (`assets/vfx/<effect>/` sessions + render folder watch) | 6 |
| §8 Browser | Multi-tab in-app browser, persistent session, Envato preload | 7 |
| §8 | Download interception → `assets/` filing + license sidecar + unzip + fallback | 7 |
| §9 Thumbnails | PSD scaffold from bundled templates; open Photoshop; PNG folder watch | 7 |
| §10 Shell | Tray (live state) · hide-to-tray · launch-at-login · single instance | 1 |
| §10 | ⌘K palette covering every app action (registry grows per phase) | 1 (framework) → all |
| §10 | Onboarding (root, ElevenLabs key, claude detection) | 1 |
| §10 | Jobs framework ("nothing fails silently") | 1 |
| §10 | Notifications with deep-link click-through | 2 |
| §10 | Design-polish ("joy") pass | 7 |
| §10 | Packaging: `.dmg`, launch-at-login from bundle | 7 |

## TODO.md milestone cross-reference

The pre-Studio-OS `TODO.md` milestones map into phases as:

| Milestone | Status / phase |
|---|---|
| M0 workspace restructure | ✅ done |
| M1 project bundle + import | Phase 4 |
| M2 transcription | Phase 4 |
| M3 cut planner | Phase 4 |
| M4 export (FCPXML, MP4, captions) | Phase 5 |
| M5 app: load + view | Phases 4–5 |
| M6 app: edit | Phase 5 |
| M7 app: AI runtime + first-run | Phase 4 (runtime) + 1 (first-run) + 6 (dock re-route) |
| M8 polish + ship | Phase 7 |

## Status tracker

Update the table as phases land (link the PR that closed each).

| Phase | Title | Status | PR |
|---|---|---|---|
| 0 | PRDs + guidance layer + repo identity | done | [#3](https://github.com/Konadu-Akwasi-Akuoko/katto/pull/3) |
| 1 | Shell & First Light | done (exit criteria passed 2026-07-09; merged to main via the phase-2 branch) | [#9](https://github.com/Konadu-Akwasi-Akuoko/katto/pull/9) |
| 2 | Projects & Planner | done (exit criteria signed off 2026-07-17) | [#9](https://github.com/Konadu-Akwasi-Akuoko/katto/pull/9) |
| 3 | SD Ingest | implemented 2026-07-22 (overnight run); pending owner hardware verification | — |
| 4 | Cut Pipeline | implemented 2026-07-22 (overnight run); pending owner verification | — |
| 5 | Cut Editor & Export | implemented 2026-07-22 (overnight run); pending owner verification | — |
| 6 | Claude Dock & Automations | not started | — |
| 7 | Browser, Thumbnails, Resolve, Import & Ship | not started | — |
