# Overnight run — phases 3 → 5 (started 2026-07-22, branch `feat/phase-3-5-ingest-to-export`)

Untracked log of the autonomous run. Every user-testable item lands here as a checkbox —
tick them as you verify after waking. Automated coverage (`just check`) is noted per phase
but is not a substitute for the manual checks below.

## Run log

- [x] Branch `feat/phase-3-5-ingest-to-export` created off `main`
- [x] Overnight instructions persisted in `CLAUDE.md` (delete that section when you wake)
- DateInput leftover work left uncommitted, excluded from all commits

## Phase 3 — SD Ingest

_Status: done — implemented, adversarially reviewed (2 HIGH + 14 further findings fixed), `just check` verified green by team lead at 05:44. Awaiting only the owner's manual checks below._

### Owner visual/manual checks (tick after testing)

- [ ] Insert the real Lexar V90 card from the ZV-E10 II → a "Camera card ready" notification appears (dev builds degrade to a `notification_degraded` events row instead of a banner).
- [ ] Click the notification (or open `katto://ingest`, or run "Import from camera card…" from ⌘K) → the import sheet opens with clips grouped by CLIP/SUB, videos pre-selected, sidecars (.XML/.THM) hidden.
- [ ] The project selector defaults to the project whose shoot date is nearest today; the primary button reads "Import to <project>".
- [ ] "New project" inline in the sheet: type a title → Create → the new project is selected.
- [ ] Click Import → the sheet swaps to the copy panel ("Copying N clips → <project>") with a live 4px bar; the tray mirrors the percentage; the dashboard jobs list shows the `ingest` job.
- [ ] On completion → footage lands in `Projects/<slug>/footage/` renamed `YYYY-MM-DD_NNN.ext` (date = project shoot date if set, else today), sequence continuing from any existing files for that date; the card is byte-for-byte unchanged (spot-check a file size).
- [ ] "Eject card" button appears on completion → clicking it ejects the card (Finder shows it gone); a second insert re-detects cleanly.
- [ ] Card removal without eject → the sheet's stale offer disappears (CardRemoved broadcast).
- [ ] Free-space guard: with a nearly-full studio drive, the sheet shows the warn callout with exact numbers, and `start_ingest` refuses server-side with "insufficient free space: need X bytes, Y free"; nothing is copied.
- [ ] Failure path: yank the card mid-copy → the job goes `failed` (visible in dashboard + events), a `<name>.partial` stays quarantined in `footage/`, completed files remain, the card is untouched.
- [ ] iPhone path: on a project detail page, drag a `.mov` from the iPhone folder onto the Footage card → same rename+verify pipeline runs, toast "Importing N files", no watcher involvement. (Note: the drop listener is window-wide on the detail view — check a stray drop elsewhere on that page doesn't surprise you.)
- [ ] `events` log shows an `ingested {count, bytes, project}` row after a successful import.
- [ ] Hardware checkpoint not covered by tests: the `/Volumes` watcher itself (notify create/remove events, 1 s readability debounce) — only exercised by a real mount.
- [ ] Not verifiable without hardware: `diskutil eject` success/failure surfaces as toast; ffprobe durations appear per clip in the sheet (dev machine has ffprobe, card clips needed).
- [ ] Fast detection: the sheet opens immediately with sizes but blank durations; durations fill in a moment later (background ffprobe, 4 at a time) — verify the refresh does not disturb a selection you already changed.
- [ ] Retry remainder: force a failure mid-copy (yank card) → the progress panel shows the terminal error and, after re-inserting the card, "Retry remaining N clips" re-imports only what didn't land.
- [ ] Failed-verify quarantine: any file failing the final size check is renamed back to `<name>.partial` — footage/ never keeps a bad file under a valid final name.
- [ ] Concurrent import safety: start an import, immediately start a second into the same project (e.g. drag-in) → the second appears immediately as its own queued `ingest` job in the dashboard and only starts copying after the first finishes; no overwritten or duplicated sequence numbers.
- [ ] Sidecars (.XML/.THM) are listed (dimmed, deselected); ticking one imports it through the same rename pipeline.
- [ ] Opening the sheet with no card (⌘K "Import from camera card…" or katto://ingest) shows "No camera card detected" instead of nothing.
- [ ] Mount a large NON-camera volume (e.g. a backup drive) → no notification, and no long disk churn (recognition probes only marker paths now).
- [ ] Eject guard: eject only works for the offered card volume — confirm a failed eject surfaces as a toast with the diskutil error.
- [ ] Tray pulse on detection is NOT implemented (deviation, below) — confirm the notification + auto-opening sheet is enough signal, or ask for the icon-swap in review.

Automated coverage: engine recognize/enumerate/naming/verify + ffprobe parser (25 tests incl. fixture-tree integration), copy job quarantine/overwrite-refusal tempdir tests, deep-link route, sheet behavior tests (selection → `start_ingest` payload → progress panel), model unit tests. `just check` green at phase end.

Design deviations from the plan (for review): `CardDetected` broadcast is payload-free (offer flows through the `card_offer` query; specta's nested event mapper broke `noImplicitAny`); ingest-sheet store lives in `src/stores/` (shared, reachable from router + palette without feature→shared violation); the import sheet swaps to the progress panel instead of closing (otherwise Eject had no home); FootageCard wiring into `project-detail.tsx` was done via the stash procedure and is committed.

Post-review deviations (2nd pass):
- **Tray pulse on card detection is not implemented.** The PRD asks for an icon swap/badge; the repo ships a single template tray icon (`icons/tray/menubar.png`) and generating a badged variant overnight would be guesswork against the design system. Signal today = notification + auto-opening sheet. If you want the pulse, supply/approve a badged template asset and it's a two-line change in `tray.rs`.
- **A second import into the same project queues as its own job (not rejected, not blocking).** The command returns a job row instantly; the per-footage-dir lock is acquired inside the job future and held to job end, and sequence planning + the authoritative free-space check run under that lock — so the queued job plans against whatever the first import actually landed. The command keeps a fast best-effort free-space precheck (no lock) so an obviously oversized import is still refused up front with exact numbers.
- `duration_s: f64` in the engine's `MediaInfo` stands (display-only model boundary; fps stays `Rational`).

## Phase 4 — Cut Pipeline

_Status: done — implemented, adversarially reviewed (2 HIGH + 15 further findings fixed across 12 fix commits), `just check` verified green by team lead at 07:31. Awaiting only the owner's manual checks below._

### Owner visual/manual checks (tick after testing)

- [ ] With the dev app running and a project containing real footage: the Footage card
      lists each clip with "Plan rough cut"; clicking it starts the three-step indicator
      (Extracting audio → Transcribing → Detecting cuts) with a live elapsed timer and
      the tray/dashboard mirroring the `cut_pipeline` job.
- [ ] Transcript appears in the review surface as soon as transcription lands (before
      planning finishes — the "Cut plans" list shows the bundle with its transcript dot
      lit); cut count ticks up during Detecting cuts (subprocess mode).
- [ ] REAL-KEY CHECK: ElevenLabs key from onboarding is used (no key → typed
      `missing_key` error with fix copy, no job started). Requires the real ElevenLabs
      key in the keychain.
- [ ] REAL-CLAUDE CHECK: with `claude` on PATH the subprocess planner runs on
      subscription auth (no ANTHROPIC_API_KEY involved); `katto auth status` shows the
      detection. Kill/rename claude → planner falls back to typed error offering API-key
      mode (store an Anthropic key in Settings/onboarding, re-run → HTTP planner with
      `planner_model` setting, default claude-sonnet-4-6).
- [ ] HARDWARE CHECKPOINT (PRD exit criterion): drag a real ZV-E10 II 4K clip through
      ingest, then run the full pipeline on it — transcript + validated cut plan visible
      in-app; words click-seek the video; cuts gray-struck, discretionary amber-dotted
      (note on hover), flags highlighted and click-to-seek only.
- [ ] `<project>/audio/<clip>.kruproj/` contains project.json, cached_audio.wav (mono
      16 kHz), transcript.json (raw Scribe body), cuts.json — and no `.tmp` litter.
- [ ] `katto cut <video>` (CLI, binary name `katto`) produces the same bundle headless;
      `katto auth status` and `--json` output look right; exit codes: usage error 2,
      missing file 1.
- [ ] `events` log shows `rough_cut_planned {bundle, cuts, flags}` after a successful run.
- [ ] Failure paths: yank network mid-transcription → job fails with typed ElevenLabs
      error, no partial transcript.json in the bundle; feed a deliberately huge clip and
      cancel is NOT available (by design — jobs run to completion; confirm this is
      acceptable or file for Phase 5).
- [ ] Ignored tests to run by hand when convenient:
      `KATTO_TEST_CLIP=/path/to/clip.mp4 cargo test -p katto-engine import_real_clip_end_to_end -- --ignored`
      and the `full_pipeline_real_binaries` test in `tests/pipeline.rs` (needs
      ELEVENLABS_API_KEY exported and claude on PATH).

Automated coverage: Rational property tests, probe-timing fixtures (incl. 29.97/59.94),
schema round-trips, 6-fixture validator table, exhaustive merge tests, bundle round-trip,
transcribe/HTTP-planner wiremock suites, stream-accum + partial-extractor fixtures
(stream-json shape verified against a live claude 2.1.217 run), retry-loop stub tests,
mocked pipeline integration, real-ffmpeg import test (run green locally against a
synthesized clip), CLI snapshots + exit codes, pipeline reducer + editor model/component
tests. `just check` green at phase end.

Design deviations from the plan (for review):
- `extract_audio_argv` pins `-f wav` explicitly — the real-ffmpeg ignored test caught
  that ffmpeg cannot infer the muxer from the `.wav.tmp` temp name. Both argv builders
  exclude the program name (the plan's ffmpeg test had it as argv[0], its ffprobe one
  didn't; made consistent).
- ffprobe fixtures live under `tests/fixtures/ffprobe/` (Phase-3 convention), not flat.
- `Cuts` deserializes through a private shadow struct: absent `discretionary`/`flags`
  still default to empty, but the specta TS export keeps the fields required — specta's
  semantic mapper emits unguarded access on optional fields (tsc break).
- specta BigInt policy: `Rational.num` (i64) exports as `number` via
  `#[specta(type = f64)]`; edits indices (usize) export as u32.
- `PlanSteps` lives in `src/features/projects/detail/` (plan said `features/pipeline/`):
  the committed one-way import rule forbids a projects→pipeline feature import; the
  pipeline run store is shared in `src/stores/pipeline.ts`.
- New `list_footage` command + clip list in the Footage card: Phase 3 left the card as a
  pure drop zone with no per-clip rows, so the "Plan rough cut" action needed a listing.
  The "Cut plans" (bundles) list also renders inside the Footage card — this avoids
  editing `project-detail.tsx`, which still carries your uncommitted DateInput hunks.
- Editor navigation is ui-store state (`editorBundlePath` + `openCutEditor`/
  `closeCutEditor`), not a URL route — the app has no router; surfaces are store-driven.
  ⌘K gains "Close cut review"; opening a specific bundle from the palette is deferred
  (needs a picker dialog like go-to-project).
- Two new app error kinds `missing_key` / `no_planner` so the UI can discriminate the
  "offer API-key mode" copy from generic engine errors.
- CLI binary is named `katto` (`[[bin]]`); CLI keychain reads degrade to "missing" on
  any error; CLI tests inject env keys so headless runs never trigger a keychain ACL
  prompt.

Post-review deviations (2nd pass):
- Asset protocol added mid-phase (team-lead heads-up): `assetProtocol` enabled with an
  empty static scope + `protocol-asset` feature; the studio root is granted at runtime
  on launch and again when `studio_root` changes, so the review surface's video pane
  (`convertFileSrc`) plays footage without a relaunch.
- rust-reviewer fixes: `kill_on_drop` + concurrent stderr drain on the claude
  subprocess; Rational `Ord` now den-tie-breaks so `Equal` ⟺ structural equality;
  `parse_exact_decimal` sign + multi-byte-slice fixes; failed `rough_cut_planned`
  event writes are logged instead of silently dropped; planner default model deduped
  to the engine constant. Left as-is: `CutsInvalid` variant (Phase-5 groundwork),
  `which claude` helper triplication (nit).
- frontend-reviewer fixes: project navigation clears `editorBundlePath` (no stale
  editor across projects); missing bundle sources get a dedicated `source_missing`
  error kind (editor copy discriminates on kind, not message prefix); pipeline types
  re-exported through `lib/ipc/pipeline` so features never import `bindings.gen`;
  editor fixtures moved to `src/test/fixtures/editor.ts`. Left as-is: a `duration-300`
  literal in plan-steps — the repo has no motion-duration Tailwind token yet
  (pre-existing gap, worth a token pass later).

Consolidated review batch (3rd pass, team-lead's merged fix list — 12 commits):
- Blockers: planner POST and Scribe upload get connect + overall timeouts (upload cap
  scales with file size); the claude correction turn re-pins the cut-decider system
  prompt (previously it resumed under the stock Claude Code prompt with tools, cwd =
  the bundle dir).
- Engine correctness: `Rational` rejects `den == 0` on deserialize and
  rescale/snap are `checked_*` (no silent i64 wrap); `validate_cuts` cross-checks the
  model's `source_duration_secs` against the transcript's measured duration (1s
  tolerance); WAV uploads stream from disk with a 5.0 GB precheck (cap verified
  against ElevenLabs docs); 401 bodies never reach error copy; a valid cached
  transcript.json is reused so planner retries never re-bill ElevenLabs;
  `write_atomic` fsyncs before rename and retries once (PRD disk-full rule).
- App: pipeline job future moved to `jobs/pipeline.rs` (commands are thin shells
  again); `PipelineEvent::Failed` carries a `FailureKind` so UI copy distinguishes
  re-enter-key from retry-later; DB-backed guard refuses a second active run on the
  same bundle (`pipeline_busy`); `open_bundle` requires the path inside the studio
  root; `SourceMissing` crosses IPC with structured fields; claude probe + keychain
  constants deduped into `katto_engine::detect`; asset-scope policy moved to an
  `assets` module.
- Frontend: "Review transcript" appears mid-run as soon as `transcript_ready` fires
  and the editor hydrates incremental cuts live until cuts.json lands (PRD gap);
  bundle/jobs queries invalidate on transcript_ready/done/failed; transcript overlays
  are a merge walk with the pane memoized and spacing tokens no longer focusable
  buttons; drag-drop listener no longer resubscribes per mutation flip.
- Recorded deviations: a REPLACED studio root stays readable over asset:// until
  relaunch (tauri's scope API has no un-allow, and forbid would permanently outrank a
  re-allow — single-user app, accepted); design debt: `src/styles/main.css` defines no
  `--dur*`/`--ease` motion tokens, so new UI (plan-steps width transition) joins the
  existing literal-duration pattern — worth a repo-wide token pass.

Owner note — keychain ACL: during CLI probing this run, macOS may have flashed a
keychain permission prompt (an unsigned dev binary reading the "katto" keychain item;
the process was killed rather than answered). Nothing was granted. Headless CLI runs
avoid the prompt by passing keys via env (`ELEVENLABS_API_KEY` / `ANTHROPIC_API_KEY`);
the CLI resolves env first and only then touches the keychain.

### Owner checks added by the review batch

- [ ] Mid-run review: start a rough cut, click "Review transcript" while Detecting
      cuts is still running — transcript is readable/click-seekable immediately and
      cut strikethroughs appear incrementally, then the persisted plan takes over.
- [ ] Start the same clip's rough cut twice quickly — the second attempt shows the
      "already in progress" error, no duplicate job row.
- [ ] Fail a run with a bad ElevenLabs key — the failure copy says the key was
      rejected and points at Settings (not a raw transport error).

## Phase 5 — Cut Editor & Export

_Status: done — implemented, adversarially reviewed (2 HIGH + 13 further findings fixed across 4 fix commits), `just check` verified green by team lead at 09:40. Awaiting only the owner's manual checks below._

### Owner visual/manual checks (tick after testing)

- [ ] EDITOR WALKTHROUGH: open a planned bundle — three panes render (transcript left,
      video right, timeline+waveform strip below); click a struck word -> cut toggles
      off everywhere (transcript, timeline, waveform); click an amber span -> becomes a
      real cut; select a sentence, press X -> manual cut snapped outward into spacing;
      click a yellow flag -> seek only.
- [ ] KEPT-ONLY PLAYBACK: playback skips cuts audibly/visibly; "Show original" restores
      everything; Space/J/K/L behave (L twice = 2x, J = reverse scrub), arrow frame
      steps land on frames, Shift+arrows jump 10.
- [ ] TIMELINE: thumbnails populate (auto-generated on first open); drag a cut edge —
      snaps to word boundaries, Option = free drag; drag-select empty area = manual
      cut on release; region click highlights + scrolls the transcript span.
- [ ] UNDO ACROSS RESTART: make edits, Cmd+Z a few times, quit the app fully, reopen
      the bundle — Cmd+Z / Cmd+Shift+Z continue exactly where they left off (history
      lives in edits.json).
- [ ] AUTO-SAVE: edits persist after an immediate window close (close guard flushes);
      edits.json shows no .tmp litter.
- [ ] EXPORT + FCP IMPORT (the big one): first export forces an NLE pick; second export
      pre-selects it; `timelines/<slug>-vN.fcpxml` + .srt + .vtt land, earlier versions
      untouched. Import the fcpxml into Final Cut: kept clips on the primary storyline,
      EVERY removed segment present as a dimmed (disabled) connected clip on the lane
      below — select one, press V, it plays again. Also import into Resolve Studio:
      clip count/durations/frame alignment match.
- [ ] DTD GATE: `cargo test -p katto-engine fcpxml -- --ignored` — already run green
      on this machine during the overnight build (xmllint against FCP's shipped
      FCPXMLv1_11.dtd, validated via a space-free copy of the DTD); re-run if the
      emitter changes.
- [ ] MP4 RENDER: "Also render MP4" spawns a jobs-row render; output plays in
      QuickTime, cuts absent, audio in sync at cut points; job failure (e.g. yank the
      source) surfaces stderr tail, bundle intact.
- [ ] CAPTIONS SANITY: open the .srt/.vtt — timestamps are kept-time (first caption at
      ~0), lines break at sentences / ~42 chars, no text from removed spans.
- [ ] RELOCATION: move the source file, reopen the bundle -> the open error offers
      "Locate the source…" naming the missing file; picking the moved file (same name)
      heals the bundle; picking a wrong file is refused with the failed check named.
- [ ] CLI PARITY: `katto export <bundle>` and `katto render <bundle>` produce the same
      artifacts headless; `katto render` on an everything-cut bundle fails loud.
- [ ] Events log shows `timeline_exported {slug, version}` rows; render/thumbs appear
      as jobs with tray progress.
- [ ] Ignored hardware tests when convenient:
      `KATTO_TEST_CLIP=/path/clip.mp4 cargo test -p katto-engine -- --ignored`
      (render_real_clip_end_to_end, generate_thumbs_real_clip).

Design deviations from the plan (for review):

- Persisted edit times (manual cuts, boundary adjustments) serialize in a microsecond
  timebase, not the frame grid — the plan's own round-trip test demands exact
  round-trips of non-frame-aligned (token-snapped) values, and the engine rescales +
  frame-snaps at merge anyway. `secondsToRational` stays frame-exact for grid math.
- `write_document` (FCPXML builder) returns `Result<String>`, not `String` — quick-xml's
  writer is fallible and the engine bans unwrap; `FcpxmlDoc.width/height` are `Option`
  so the emitter can omit unprobed dimensions (plan's Task-6 decision, one struct).
- No `tauri.conf.json` change: phase 4 already enabled the asset protocol with a
  runtime studio-root allow (src-tauri/src/assets.rs), which covers video, wav, and
  thumbnails. Scope stays tight instead of the plan's `$HOME/** + /Volumes/**`.
- `project_context` lives in `katto_engine::timelines` (shared by CLI and app) instead
  of CLI-only.
- Relocation picking goes through a new `pick_relocation_file` command (native Rust-side
  picker, same pattern as `pick_studio_root`) — the plan left the mechanism open.
- The thumbs job reports its count via the final progress message, not the done payload
  (the jobs framework's payload is fixed at spawn).
- Timeline drag-select creates the manual cut on pointer release (≥0.05 s), no X press
  required — X-with-selection stays the transcript pane's flow.
- The DTD gate test copies the DTD to a temp path first: xmllint treats `--dtdvalid` as
  a URI and the spaces in "Final Cut Pro.app" break entity resolution.

Post-review hardening (adversarial review round, commits 50b9835..7e8fbfb):

- Rescue clips sharing a parent now stack on lanes -1, -2, … (FCP rejects overlapping
  same-lane items); fcpxml_rescue snapshot updated and re-validated against the DTD.
- `render_mp4` no longer takes an output path from the frontend; the backend always
  allocates the next exports/<slug>-render-vN.mp4 and a DB-backed busy guard refuses a
  second concurrent render of the same bundle. `open_in_fcp`/`reveal_timeline` are now
  containment-checked against the studio root.
- Relocation logic moved into `katto_engine::bundle` (typed `Relocate` error preserved
  across the IPC error mapping).
- Frontend: canonical disc-N keys for unapplied discretionary regions; waveform never
  removes a region mid-drag (undo history can no longer be stranded paused); transcript
  opts into text selection via `.select-text` (body user-select:none had silently
  disabled drag-select+X outside jsdom — worth a manual check); editor store survives
  query refetches; transport keys suspended under dialogs; backward stepping lands
  before cuts; autosave runs serialized; close guard also flushes on paused-error;
  waveform drags token-snap; filmstrip redraws on image decode.

- [ ] REVIEW-FIX SPOT CHECK: drag a discretionary region's edge in one motion (it
      converts to a manual cut mid-drag) then undo — exactly one undo step, history
      still live. Apply the SECOND of two discretionary suggestions from the timeline
      and confirm the right span strikes. Select transcript text by mouse and press X.

Final consolidated review pass (commits 0bf2e64..bc62e20):

Fixed: relocated/outside-root sources now reach the asset-protocol scope (grant at
relocation + re-grant on every bundle open); autosave can no longer report idle over an
edit made during an in-flight save (close-guard silent-loss race) and a paused save
genuinely retries on the next edit; versions are reserved with create_new (scan errors
surface, failed exports/renders release their claim, render slugs are per-bundle);
stranded queued/running jobs fail over at launch; ffmpeg gets kill_on_drop + a 120s
stall watchdog; relocation probes through the app's single ffprobe site; caption
midpoints are exact rational math and cues never overlap their predecessor; transcript
X/Delete is suspended behind dialogs; edge drags clamp at one frame; export-dialog
rejections and flush failures surface inline; thumbs swap via rename-aside; transport
hook fully unit-tested; radio-group/slider primitives added and used.

Accepted (recorded, no action): edits.json drops unknown fields on rewrite (single-user
forward-compat nit); dialog errors render inline via local try/catch (PRD-required UX,
not the toast path); wavesurfer decodes the full audio into memory (covered by the
long-clip owner checkpoint).

- [ ] RELOCATION OUTSIDE THE ROOT: relocate a source to a folder OUTSIDE the studio
      root (e.g. ~/Desktop), pick it — video must actually play, immediately and again
      after relaunching katto.
- [ ] CRASH RECOVERY: force-quit katto mid-render; relaunch — the job shows failed
      ("interrupted at launch") with an events row, and a new render of the same bundle
      starts instead of reporting busy.
- [ ] SAVE-FAILURE RETRY: with the auto-save banner up (e.g. make the bundle dir
      read-only, edit, restore permissions), the NEXT edit clears the banner by itself;
      closing the window with the banner up still makes one flush attempt.

---

# Overnight run 2 — phases 6 → 7 (started 2026-07-22 ~10:15, branch `feat/phase-6-7-dock-to-ship`, stacked on the phase-3-5 branch)

## Run log

- [x] Branch `feat/phase-6-7-dock-to-ship` created off `feat/phase-3-5-ingest-to-export`
- [x] CLAUDE.md TEMPORARY section updated for run 2 (delete the whole section when you wake)
- Same protections: DateInput work stays uncommitted; nothing pushed; ship-skill commits

## Phase 6 — Claude Dock & Automations

_Status: implemented + review-hardened — all 20 plan tasks done
(`docs/plans/2026-07-22-phase-6-claude-dock.md`), then the team lead's consolidated
three-review fix list (3 blockers + 10 further items) landed across 9 fix commits
(93bdf7f..7d14dfc). `just check` green after the fixes. Awaiting the owner's manual
checks below._

### Owner visual/manual checks (tick after testing)

- [ ] DOCK WALKTHROUGH: sidebar Claude icon → the panel slides over the content pane;
      "New session" opens a live claude tab (xterm, WebGL); type into it mid-run; hide
      the panel while it works (icon pulses); reopen — scrollback intact (2 MiB ring).
- [ ] ICON STATES: running = pulse ring; needs-input = warn badge (trigger a permission
      prompt, e.g. ask claude to run a non-allowed command); done = brief check flash on
      running→idle; with the panel hidden, a needs-input notification arrives and
      clicking it opens the dock focused on that session.
- [ ] CUT PLAN VIA DOCK: "Plan rough cut" on a project with footage → a "cut plan: …"
      session tab appears and visibly works → cuts.json lands → the pipeline continues
      (three-step indicator completes). One invalid plan gets a single correction turn
      (cuts.invalid-1.json left in the bundle); a second invalid fails typed. Toggle
      Settings → "Cut planning in the dock" OFF → the old subprocess path still works.
- [ ] IDLE REAPING: set idle timeout to 2 min in Settings → leave a session idle past
      it → its tab shows "closed after idle"; the session focused in an OPEN panel is
      spared; the events log records the reap.
- [ ] NIGHTLY CURATION DRY RUN: Settings → Automations → "Run now" → a curation session
      judges the raw-signal delta (subscription auth, no API key) → notification
      "kept N / discarded M" → clicking it lands on the Planner backlog; new curated
      ideas show rationale + "suggested" kind affix (hover = the AI's why) + the 3-bar
      lean notch + source domain link; changing OR check-confirming a kind flips its
      provenance to human (affix disappears). Discovery toggle: leave OFF unless
      `uv`/hyper-frames is set up — with it ON the run shells `studio-discover` first
      (was NOT executed during the overnight run).
- [ ] SCHEDULER CATCH-UP: set curation to a time the Mac will sleep through, sleep it,
      wake after the slot (within the 20 h catch-up window) → the run fires once (check
      `scheduler_manual_run`/`curation_done` events), and does not run again that day.
- [ ] VFX FLOW: project detail → Effects card → "New effect" → a session opens cwd'd to
      `assets/vfx/<effect-slug>/` and the dock focuses it; drop or render an .mp4 into
      that folder → within ~a minute it appears as the tile's preview video + an events
      row (`vfx_render_landed`); the size-stability debounce means a still-encoding file
      waits until it stops growing.
- [ ] SESSION HYGIENE: while a session runs, `ls ~/Library/Application Support/katto/`
      (or the dev-mode app-data dir) shows a per-session `hook-settings-*.json` (0600);
      after the session closes it is deleted. `lsof -i` shows the hooks endpoint bound
      to 127.0.0.1 only.
- [ ] Real SD-card checkpoint from phases 3–5: still pending above (not duplicated here).

### Owner checks added by the review batch

- [ ] SUBSCRIPTION-AUTH GUARD: `export ANTHROPIC_API_KEY=sk-test-junk` in your shell
      profile (temporarily), launch katto from that environment, open a dock session and
      run `echo ${ANTHROPIC_API_KEY:-empty}` → must print `empty` (the scrub removes it
      at spawn AND after zsh profile sourcing). Remove the test export after.
- [ ] HIDDEN-DOCK STREAMING: start a chatty session (e.g. ask claude to tail something),
      hide the dock for a few minutes, reopen — scrollback intact, and Activity Monitor
      shows no runaway memory growth in katto while hidden (the backend now drops the
      stream sink the moment the terminal unmounts).
- [ ] CLOSED-SESSION TYPING: let a session close (idle reap or close it from another
      surface) while its tab is still visible, type into it — a one-line "input not
      delivered" notice appears in the viewport instead of silent nothing.
- [ ] DONE-FLASH: with two sessions running, let one finish while the other keeps
      producing updates — the check flash still clears after ~3 s (previously an
      unrelated update could strand it lit).
- [ ] RE-PLAN FRESHNESS: run "Plan rough cut" twice on the same clip — the second run
      moves the old plan to cuts.prev.json and a fresh session writes a new cuts.json;
      the run fails loudly rather than silently reusing last run's plan.

Known design note (accepted, recorded per review): the nightly curation session judges
rows by running sqlite3 against katto's own DB with model-constructed SQL — inherent to
the curation design (hyper-frames pattern); the blast radius is the owner's own database
and the session is permission-scoped to `Bash(sqlite3:*)`.

Automated coverage: 252 backend tests (session state machine, launch/hook-settings JSON,
scrollback ring, reap predicate, anacron due/backoff/catch-up table, plan-file verdicts,
curation prompt + delta counting, VFX slug/classify/debounce, hooks endpoint auth, ideas
kind_source, open_external_url guard; 3 `#[ignore]`d incl. a real-claude smoke test that
passed live on subscription auth), 315 frontend tests (dock state/tab strip/panel/terminal
with mocked xterm, schedule-spec parsing, dock settings section, vfx card, lean model,
backlog provenance). `just check` green at phase end (tail in the run report).

Design deviations from the plan (for review):

- `evaluate_plan_file` also runs `validate_cuts` against the transcript (plan guessed
  parse-only) — dock-planned cuts get the same validation the subprocess path had.
- `SchedulerHandle` is `app.manage()`d separately instead of living in `AppState`
  (construction order: the scheduler needs the app handle, AppState is built before it).
- The idle-reap control moved from Settings→General into the new Dock section (single
  source; General no longer shows it).
- Sidebar/AppShell take `dock`/`overlay` slot props filled from `src/app/app.tsx` rather
  than importing dock components (committed one-way import rule).
- VFX components (`vfx-card`, `new-effect-dialog`) live in
  `src/features/projects/detail/` next to `footage-card` — the plan's
  `src/features/vfx/` location would have created the repo's only feature→feature
  import (projects→vfx). The IPC wrapper stays at `src/lib/ipc/vfx.ts`.
- The real-claude smoke test asserts an arithmetic answer (31217+11202) instead of
  "pong" — the prompt echo contained the expected token, passing without a model turn.
- `spawn_session`'s IPC surface (`NewSession`) exposes only label/cwd/initial_prompt;
  permission-mode/allow-rules stay backend-internal (curation + cut-plan set them).
- New `open_external_url` command (http/https only) for the backlog source links —
  no opener existed for URLs, only reveal-in-Finder.
- `IdeaPatch` gained `kind_source` end-to-end (repo SQL, command struct, bindings) so
  the human's kind decision is recorded as provenance.

## Phase 7 — Browser, Thumbnails, Resolve, Import & Ship

_Status: done — implemented, adversarially reviewed (1 CRITICAL + 1 security HIGH + 19 further findings fixed across 14 fix commits), `just check` verified green by team lead at 19:15, .dmg rebuilt. Awaiting only the owner's manual checks. **This is the final phase — the whole app is feature-complete.**_

### Owner checkboxes — Phase 7

Browser:

- [ ] Open the Browser surface → Envato Elements loads in the default tab, already
  logged in from the persisted session (log in once if this is the very first run);
  quit katto fully, relaunch → still logged in.
- [ ] Tabs: new tab, close tab, switch tabs (only the active tab's page visible);
  back/forward enable and disable correctly; address bar navigates (bare
  `example.com` works, plain words are refused with the inline hint).
- [ ] REAL ENVATO DOWNLOAD (the big one): download a texture pack from Envato → it
  never appears in ~/Downloads; lands unzipped in
  `<active project>/assets/envato/<item>/` with `<item>.license.json` beside it;
  toast names the project + path; events log shows `asset_filed`; the downloads
  popover row offers Reveal.
- [ ] BLOB EDGE CASE: trigger a blob:-style download on Envato (some preview/export
  buttons) → file appears in ~/Downloads plus the persistent "couldn't intercept"
  notice (PRD D16 fallback).
- [ ] No-project flow: with a fresh studio root (or after clearing the active
  project), download something → the "pick a project" sheet appears; filing proceeds
  after the pick; Discard genuinely drops the file.
- [ ] Non-Envato download files to `assets/` (not `assets/envato/`).
- [ ] Window close → reopen (tray) with the browser surface: tabs survive, page
  state reloads (expected — webviews are rebuilt).
- [ ] Fallback host: set `browser_single_webview` to true in settings (or after an
  automatic `browser_fallback_engaged` event) → relaunch → tabs still work through
  one reused webview.
- [ ] ⌘K note: the palette shortcut doesn't fire while a web page has focus (the
  webview owns the keyboard) — click katto chrome first. Confirm this is acceptable.

Thumbnails:

- [ ] "New thumbnail" → picker → PSD lands as `thumbnails/<slug>-thumb-a.psd` and
  Photoshop opens it (with Photoshop absent/renamed: Finder reveal instead, and the
  toast says so); guides visible in Photoshop at the safe-zone lines (View → Show →
  Guides); second scaffold is `-b`; export a PNG into `thumbnails/` → the project
  card and the Thumbnails card show it within ~a second.
- [ ] Optional: replace `src-tauri/resources/thumbnail-templates/*.psd` with
  hand-authored templates (same filenames) — rebuild picks them up.

Resolve (Studio required):

- [ ] With Resolve Studio running and external scripting = Local: "Open in Resolve"
  on an exported project → project created, timeline + media in the pool.
- [ ] With Resolve closed → the exact "isn't running" remedy.
- [ ] With free Resolve or scripting off → the scripting-unavailable remedy naming
  both causes.

studio.db import:

- [ ] Settings → Import from studio.db → default path is right → Dry run shows
  plausible counts → Import → ideas appear in the backlog with statuses mapped
  (spot-check a promoted idea keeps its exact slug); re-run → all "unchanged",
  nothing duplicated; events row `studio_imported`.

Polish:

- [ ] Motion feels uniform (fast hovers, calm panels); System Settings →
  Accessibility → Reduce Motion kills the promote flash/dock pulse/slides.
- [ ] Tab-walk the browser toolbar, import wizard, template picker — every stop
  shows the ember ring; no hand cursors anywhere except real links.

Packaging (the finale):

- [ ] Install the dmg (artifact path below) on a clean profile → onboarding →
  end-to-end loop: idea → promote → (skip shoot) drag a clip in → rough cut →
  export → browser download files itself → thumbnail scaffolds — without leaving
  katto except FCP/Photoshop.
- [ ] Launch-at-login from the installed app survives a reboot.
- [ ] Dock icon + tray both present (D21).

### Packaging artifact

- dmg: `target/release/bundle/dmg/katto_0.3.0_aarch64.dmg` (9.2 MB, rebuilt 2026-07-22 19:13 — includes the full review fix pass; the 18:23 build is superseded)
- app: `target/release/bundle/macos/katto.app` — `codesign -dv`: `flags=0x20002(adhoc,linker-signed)` (unsigned/ad-hoc as documented in docs/packaging.md)
- Bundled resources verified: both thumbnail-template PSDs present under `Contents/Resources/resources/thumbnail-templates/`
- Note: bundles land in the workspace-root `target/` (shared cargo target dir), not `src-tauri/target/` as the plan's path implied

### Review fix pass (19:10) — 14 commits after the three consolidated reviews

Both merge-blockers and all highs landed: overlays now hide the native webview
(C1); the recursive studio-root asset grant is gone — per-use grants only (H1,
security); updates-only re-imports can apply (H2); thumb re-exports cache-bust
by mtime (H3); filing rows resolve via download_id (H4); python3 killed on
Resolve timeout; duplicate same-URL downloads queue FIFO; parked downloads are
recoverable from the popover; import failures unwedge the wizard; address bar
keeps a mid-edit draft; preview shares apply's upserts (rolled-back txn);
Photoshop opens by bundle id; PNG deletions refresh the card; plus the lows
(single_root traversal guard, naming case-insensitivity, spawn_blocking misses,
dead exports, interpunct removal, popover shadow token, stray main.css blank
line committed). Skipped deliberately: converting the 12 hand-rolled chrome
buttons to the Button primitive (LOW; invasive mid-pass — focus rings already
unified by the audited pattern; safe to do post-review).

Additional owner checkboxes from the fixes:

- [ ] OVERLAYS ABOVE THE PAGE (was merge-blocking): with a tab loaded, open the
  downloads popover, trigger the needs-project sheet, hit ⌘K, open the dock,
  and let a toast fire — each hides the page and paints above; the page returns
  when the last overlay closes. The address hint now sits inline in the toolbar.
- [ ] ASSET SCOPE (was security-blocking): editor playback (video + waveform +
  filmstrip), vfx render previews, and thumbnail PNGs all still load after the
  per-use grant change. Then the negative test: in a browser tab's page, run
  `fetch("asset://localhost/" + encodeURIComponent("<abs path to a studio file
  you have NOT opened this session>"))` from its devtools (Safari → Develop →
  katto) — it must fail.
- [ ] Dismiss the needs-project sheet (Esc) → the popover row shows "waiting
  for a project pick" with a Choose project button that reopens the sheet; a
  second download parking while the sheet is open does not steal it.
- [ ] Download completing with no project while on the Dashboard → a toast
  offers "Choose" and jumps to the browser.
- [ ] Type in the address bar while a page redirects — the draft survives.
- [ ] Close the last tab → "The web, filed." empty state (no silent Envato
  respawn).
- [ ] Re-export a PNG over the same filename in Photoshop → the card repaints
  within ~a second; delete the newest PNG → the card falls back/empties.
- [ ] With versioned Photoshop installed ("Adobe Photoshop 2026") → "New
  thumbnail" opens Photoshop itself, not the Finder fallback.
- [ ] Re-run the studio.db import when only updates exist → the Import button
  is enabled and labeled with the update count; a failed apply shows its error
  inline instead of hanging at "Importing…".
