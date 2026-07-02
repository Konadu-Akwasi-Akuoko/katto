# Phase 5 — Cut Editor & Export

## Goal

The full transcript-primary editor (edit cuts in three synchronized panes with persistent
undo) and the deliverables: FCPXML 1.11 **with the rescue track**, optional MP4 render, and
SRT/VTT captions — versioned into `<project>/timelines/`, opening cleanly in Final Cut Pro.

## Why this order

Editing needs Phase 4's bundle + read-only surfaces; exports need edited cuts to be worth
exporting. This phase completes old milestones M4–M6 and closes the loop that makes katto
useful daily.

## User stories

- I read the transcript, click a struck word to rescue it, click an amber span to accept a
  discretionary cut, select a rambling sentence and press X to cut it.
- I play the video and hear only what survives the cuts, before any render exists.
- I drag a cut's edge on the timeline a few frames; it snaps to the nearest word boundary
  unless I hold Option.
- Cmd+Z works across app restarts.
- I export; `nvme-deep-dive-v3.fcpxml` lands in `timelines/` (v1 and v2 untouched); "Open in
  Final Cut" hands me a timeline where every removed segment sits muted on a second track —
  one click rescues any AI mistake.

## Scope with acceptance criteria

### Editor

| Feature | Acceptance criteria |
|---|---|
| Transcript editing | Click struck word → toggle that cut off; click amber span → discretionary becomes a real cut; drag-select + `X`/Delete → manual cut with boundaries snapped **outward** into adjacent spacing tokens; click yellow flag → seek only, never auto-cut |
| Kept-only playback | Default mode: `timeupdate` listener seeks past entering a cut range; "show original" toggle; transport: Space play/pause, J/K/L shuttle (1×/stop/1× reverse-ish per HTML5 constraints, L stacking 2×), ←/→ frame step, Shift+←/→ 10 frames |
| Timeline pane | Canvas track spanning source duration; frame thumbnails every 2 s (extracted once via ffmpeg into `thumbs/`, regenerable); cut regions dimmed/hatched, discretionary dotted; drag region edges frame-accurate with token snap (Option = free drag); drag-select empty area + `X` → manual cut; click region ↔ transcript span highlight + scroll |
| Waveform | wavesurfer v7 over `cached_audio.wav` (asset protocol) + regions plugin mirroring cuts. Known gotchas (from hyper-frames audio-editor, normative): only `region-update`/`region-updated` events exist; `region.updatingSide` distinguishes user drags; never `addRegion` before `ready`/non-zero duration (bounds clamp to 0); region content lives in a ShadowRoot — inject styles post-create; `part` is multi-token (`[part~="region"]`) |
| Undo/redo | One edit per step (toggle, boundary drag **coalesced per drag** — snapshot on drag start, single history entry on drag end, per audio-editor's `useCutsEditor` pattern; depth 100); Cmd+Z / Cmd+Shift+Z; history persisted in `edits.json`, survives restart |
| Auto-save | Every edit debounced 200 ms → single `save_edits` bridge call (the only interactive-path IPC); dirty guard on window close |
| Relocation | Bundle open with missing source → dialog matched by filename + duration; rewrites only the manifest's path |

### Exports (engine)

| Feature | Acceptance criteria |
|---|---|
| FCPXML 1.11 | Hand-written via quick-xml `Writer` behind a typed builder (serde fights the heterogeneous spine; DOCTYPE is manual anyway). One `<sequence>` at source frame rate; `tcFormat` DF for 29.97/59.94 else NDF; asset via percent-encoded `file:///` URL (`url::Url::from_file_path`); **every** `offset/duration/start/frameDuration` a rational `<num>/<den>s` in the format timebase — a decimal second anywhere is a validation failure |
| Rescue track | Kept segments as `<asset-clip>`s on the primary spine; **every removed segment** emitted on a second, disabled/muted lane (connected clips, `enabled="0"` semantics) so recovery is one click in FCP — new emitter invariant with its own snapshot fixture |
| Versioning | Exports land at `<project>/timelines/<slug>-v<N>.fcpxml`, N = max existing + 1; never overwrite; emitter output schema-validated before write; `events` row |
| Open in FCP | `open -a "Final Cut Pro" <file>` post-export action |
| MP4 render | cut-video pattern **verbatim** (normative source: `hyper-frames/tools/cut-video/src/cut_video/segments.py`): removed spans → coalesce → keep-windows walking 0→EOF, drop keeps ≤ 1-frame epsilon, error loud if everything removed; per-keep `[0:v]trim=S:E,setpts=PTS-STARTPTS[vi]` + `atrim/asetpts`, single `concat=n=N:v=1:a=1`; floats at 6 decimals for byte-identical graphs; graph written to a `filter_complex_script` **file** (argv-limit safe, validated against a 500+-cut fixture); always re-encode (`libx264 -crf 18 -preset medium -pix_fmt yuv420p`, aac audio, `+faststart`) — never `-c copy` |
| SRT/VTT | Kept-only retiming: drop words inside cuts; shift others left by preceding cut total; group lines at sentence boundary or 42 chars; outputs next to the timeline export |
| CLI | `katto render <bundle> -o out.mp4`, `katto export <bundle>` (FCPXML + SRT/VTT into `timelines/`) |

## Backend (Rust)

Engine additions: `emit/fcpxml.rs` (+ `emit/fcpxml/builder.rs` typed element builder),
`emit/srt.rs`, `render.rs` + `render/segments.rs` (pure keep-window + filtergraph math),
`thumbs.rs` (2 s-cadence extraction). App additions: `commands/editor.rs`
(`save_edits`, `export_timeline`, `render_mp4`, `relocate_source`, `generate_thumbs`).

## Frontend (React)

`features/editor/` completes: `store/` (zustand + zundo: document = cuts+edits effective
state; `partialize` document-only; drag coalescing via explicit `beginDrag/commitDrag`
actions), `model/` (pure: toggle/apply/manual-cut/boundary-adjust producing new edits,
kept-range computation for playback + retiming preview, snap math), `timeline-pane.tsx`
(canvas), `waveform.tsx`, `transport.ts` (keyboard map), export dialog (format checkboxes,
version preview, post-export "Open in Final Cut").

## Wiring / IPC

| Command | Notes |
|---|---|
| `save_edits(bundle_path, edits: Edits) -> ()` | debounced client-side; atomic write |
| `export_timeline(bundle_path) -> {fcpxml_path, srt_path, vtt_path, version}` | validates then writes versioned |
| `render_mp4(bundle_path, out?, Channel<JobProgress>) -> job_id` | jobs framework |
| `generate_thumbs(bundle_path, Channel<JobProgress>) -> job_id` | idempotent, regenerable |
| `relocate_source(bundle_path, new_path) -> ()` | filename+duration match enforced |
| `open_in_fcp(path)` / `reveal_timeline(slug)` | |

## Data-model deltas

None (all state in bundle files; exports recorded in `events`).

## Error handling

- Emitter validation failure → export aborted before any file write, invariant named.
- ffmpeg render failure → job failed with stderr tail; bundle intact.
- Disk full during auto-save → retry once → pause auto-save + banner until acknowledged
  (edits held in memory).
- Missing source at export → `SourceMissing` → relocation dialog.
- FCP not installed → open action falls back to reveal-in-Finder with a note.

## Testing

- **Golden snapshots (insta)**: same cuts.json in → byte-identical FCPXML out; fixtures:
  basic, rescue-track, DF 29.97, 500+ cuts, unicode filename URL-encoding. Follow the
  `emitter-snapshot-change` skill for any change.
- Keep-window + filtergraph math: table-driven parity tests against cases derived from the
  cut-video source (coalescing, epsilon drops, whole-duration-removed).
- SRT retiming units (cut-spanning words, line grouping).
- Frontend: `model/` pure tests (toggle/manual/snap/kept-ranges), undo coalescing (drag =
  one entry), store `partialize` behavior.
- Manual checkpoint: import generated FCPXML into **FCP and Resolve Studio** — clip count,
  durations, frame alignment match; rescue track present and disabled. `xmllint --dtdvalid`
  against `FCPXMLv1_11.dtd` behind `expensive-tests`.

## Out of scope

Editing transcript text (v2), multi-clip projects, B-roll/captions layers, Premiere-dialect
XML (only if FCPXML proves insufficient — D12), Resolve scripting API (Phase 7), dock
re-route (Phase 6).

## Exit criteria

Real 4K clip → transcribe → AI cuts → refine in all three panes → FCPXML opens clean in Final
Cut with both tracks; undo survives restart; `just check` green.
