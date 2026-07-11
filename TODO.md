# katto — TODO

Source of truth: the PRDs in [`prd/`](prd/) (start at [`prd/index.md`](prd/index.md)).
One phase per implementation session, from its PRD, in order — every phase is a vertical
slice with its own exit criterion. Update `prd/index.md`'s status tracker as phases land.

## Phases

- [x] **Phase 0 — PRDs, guidance layer, repo identity** · this checklist's own birth
  - [x] `.gitignore` anchoring fix (`.claude/` committable)
  - [x] `prd/` — README, index, seven phase PRDs
  - [x] `.claude/` rules · skills · hooks · reviewer agents · `CLAUDE.md` · `justfile`
  - [x] README/TODO rewritten to the Studio OS identity
- [ ] **Phase 1 — Shell & First Light** · [`prd/phase-1.md`](prd/phase-1.md)
      Tray residency, onboarding, SQLite schema, events log, jobs framework, ⌘K palette,
      frontend toolchain. *Done when: installable .app lives in the menu bar and onboarding completes.*
- [ ] **Phase 2 — Projects & Planner** · [`prd/phase-2.md`](prd/phase-2.md)
      Folder anatomy, scan/reconcile, Board/Calendar/Backlog, quick-capture hotkey, promote
      flow, notifications + deep links. *Done when: hotkey idea → promoted → real SSD folder + board card.*
- [ ] **Phase 3 — SD Ingest** · [`prd/phase-3.md`](prd/phase-3.md)
      Volume watcher, card recognition, copy/rename/verify, eject. *Done when: real Sony card
      → two clicks → verified footage in the right project.*
- [ ] **Phase 4 — Cut Pipeline** · [`prd/phase-4.md`](prd/phase-4.md)
      Rational, validators, import, transcribe, CutPlanner (subprocess + HTTP), CLI, pipeline
      UI, read-only review. *Done when: real 4K clip → transcript + AI cut plan in-app; headless via `katto cut`.*
- [ ] **Phase 5 — Cut Editor & Export** · [`prd/phase-5.md`](prd/phase-5.md)
      3-pane editing, persistent undo, FCPXML 1.11 + rescue track, MP4, SRT/VTT, versioned
      timelines. *Done when: refined FCPXML opens clean in FCP with both tracks.*
- [ ] **Phase 6 — Claude Dock & Automations** · [`prd/phase-6.md`](prd/phase-6.md)
      Session pool, terminal panel, scheduler with catch-up, nightly curation, VFX cockpit,
      cut-plan re-route. *Done when: cut plan runs visibly in a dock tab; nightly job survives sleep.*
- [ ] **Phase 7 — Browser, Thumbnails, Resolve, Import & Ship** · [`prd/phase-7.md`](prd/phase-7.md)
      Download-intercepting browser, PSD scaffolding, Resolve import, studio.db import, polish,
      `.dmg`. *Done when: the full loop runs without leaving the app except FCP/Photoshop.*

## Old milestone map (pre-Studio-OS TODO)

The original M-milestones live on inside the phases: M0 ✅ (workspace) · M1–M3 → Phase 4 ·
M4 → Phase 5 · M5 → Phases 4–5 · M6 → Phase 5 · M7 → Phases 1 + 4 + 6 · M8 → Phase 7.

## Parked issues (revisit)

- [ ] **Quick-capture hotkey doesn't surface the window over a fullscreen Space.** `⌥⌘K` fires
      and the window is built with `visible_on_all_workspaces(true)`
      (`NSWindowCollectionBehaviorCanJoinAllSpaces`), which fixes ordinary Spaces — but on a
      *fullscreen* Space the window still opens out of sight. Restarted app, still reproduces.
      Suspected fix: also set `NSWindowCollectionBehaviorFullScreenAuxiliary` on the capture
      `NSWindow` via `objc2` after build (the app already links `objc2`/`objc2-foundation`),
      and/or `MoveToActiveSpace`. See [tauri#11488](https://github.com/tauri-apps/tauri/issues/11488).
      Owner runs multiple fullscreen katto Spaces, so this is the common case for them.
      File: `src-tauri/src/capture.rs` `open_capture_window`.

## Cross-cutting checklist (apply throughout)

- Rational time end-to-end in the engine; floats only at UI/model edges
- No media bytes through `invoke` — asset protocol for audio/video/thumbs
- Long ops stream via `Channel<T>`; nothing fails silently (jobs + events)
- Folders are truth; SQLite is an index reconciled on launch
- No numeric scoring anywhere — AI suggests, the human decides
- Keychain for every secret; never log key material
- Snapshot tests freeze emitter output; versioned exports never overwritten
- The gate is `just check`; CI mirrors it 1:1
