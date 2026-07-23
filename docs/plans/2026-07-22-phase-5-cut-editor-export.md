# Phase 5 — Cut Editor & Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (or
> superpowers:subagent-driven-development) to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** The full transcript-primary editor (three synchronized panes, persistent undo,
200 ms auto-save) and the deliverables: FCPXML 1.11 with the rescue track, optional MP4
render, SRT/VTT captions — versioned into `<project>/timelines/`, opening cleanly in Final
Cut Pro (PRD: `prd/phase-5.md`).

**Architecture:** All pure logic (keep-window math, filtergraph text, caption retiming,
FCPXML building, version allocation) lands in `crates/katto-engine` as snapshot-tested
emitters; the engine also owns the thin ffmpeg spawn sites. `src-tauri` adds one
`commands/editor.rs` domain (save/export/render/thumbs/relocate/open). The frontend
completes `features/editor/`: a per-bundle zundo-wrapped document store, pure `model/`
edit ops and kept-range math, canvas timeline, wavesurfer waveform, transport, export
dialog. JS owns live edit state; the debounced auto-save is the only interactive bridge
call; media travels via the asset protocol only.

**Tech Stack:** Rust 2024 (quick-xml 0.41, url 2.5.8, insta/rstest dev), Tauri 2 +
tauri-specta `=2.0.0-rc.25`, React 19 + TS + Zustand v5 + zundo 2.3.0 +
wavesurfer.js 7.12.x, vitest + RTL, bun.

## Global Constraints

- **Gate:** `just check` (fmt-check + clippy `-D warnings` + cargo test + biome + tsc +
  vitest) from the workspace root. Never claim a task or the phase done without it green.
- **Rational end-to-end:** engine times are `Rational {num, den}`; floats only at UI and
  model/transcript boundaries — and in emitted *text* (filtergraph `%.6f`, SRT/VTT
  timestamps), formatted from Rational at the last moment.
- **Media bytes never cross `invoke`:** video and `cached_audio.wav` reach the WebView via
  `convertFileSrc` (macOS URL shape is `asset://localhost/<encoded-path>` — never
  hardcode; always call `convertFileSrc`). Thumbnails likewise.
- **JS owns live edit state.** The debounced **200 ms** auto-save calling `save_edits` is
  the only interactive-path IPC. Long ops (`render_mp4`, `generate_thumbs`) stream via
  `Channel<JobProgress>` under the jobs framework.
- **Versioned exports are never overwritten:** `timelines/<slug>-v<N>.*`, N = max existing
  + 1; every artifact write is atomic (`.tmp` → rename). Emitter output is
  schema-validated before any file write; failures name the invariant.
- **No numeric scoring** anywhere (discretionary confidence stays the `low|medium|high`
  enum, rendered as words or the amber treatment, never a number).
- **Nothing fails silently:** renders/thumbs are `jobs` rows; export writes an `events`
  row; ffmpeg failure surfaces stderr tail; disk-full on auto-save retries once then
  banners.
- **Snapshot discipline:** FCPXML/SRT/VTT/filtergraph text is insta-frozen. Any change to
  those emitters goes through the **`emitter-snapshot-change` skill**. Never commit
  `.snap.new`.
- **Phase-4 dependency:** this plan builds on the contracts in
  `docs/plans/2026-07-22-phase-4-cut-pipeline.md` (Rational, schemas, merge, bundle,
  editor read-only surfaces, CLI skeleton), which is being implemented **concurrently**.
  Every consumed phase-4 signature below is marked **[P4-CONFIRM]** — re-read the real
  file at implementation time; the phase-4 implementer may have deviated. Do not start a
  phase-5 task whose `Consumes` names phase-4 items until those items exist on the branch.
- **Dirty-tree discipline:** `src/components/ui/date-input.{tsx,test.tsx}`, hunks in
  `src/features/projects/detail/project-detail.{tsx,test.tsx}` and one hunk in
  `src/styles/main.css` are leftover DateInput work — **never commit them, never
  `git add -A`**. If a commit must touch those files: `git stash push` the DateInput
  paths, commit, `git stash pop`; or `git add -p` and verify with `git diff --cached`
  that no DateInput hunk is staged. Never stage `CLAUDE.md`, `docs/overnight-run.md`, or
  `docs/plans/`.
- **Do not start the dev app**; the owner tests visually after waking. All UI-observable
  behavior lands as checkboxes in `docs/overnight-run.md` (Task 19).
- Conventional commits, one concern per commit, tests travel with their feature commit.
  Frontend: bun only; regenerate bindings via `just bindings` / the `export_bindings`
  test; never hand-edit `src/lib/ipc/bindings.gen.ts`. Use `add-tauri-command` before new
  commands and `add-feature-surface` if a new route/surface is created.
- Engine crate keeps `#![warn(missing_docs)]`; no `unwrap()`/`expect()` outside tests;
  one thiserror enum per crate; 2018 module style (`foo.rs` + `foo/bar.rs`, no `mod.rs`).
- Design-system rules (`.claude/rules/design-system.md`) bind every UI task: tokens never
  literals, mono only for machine data (timecodes) with `tabular-nums`, `cursor:
  default`, state shown once, no eyebrows/rails/gradients/glassmorphism, grain only on
  opaque surfaces, `prefers-reduced-motion` gates all animation.

### Verified external contracts (do not re-derive)

- **FCPXML 1.11** — see the dedicated section below (`FCPXML 1.11 verified contract`).
- **wavesurfer.js 7.12.x** (verified against 7.12.11 source + npm exports, 2026-07-22;
  hyper-frames audio-editor pins `^7.12.7` — same minor line, its gotchas apply):
  imports `import WaveSurfer from "wavesurfer.js"` and
  `import RegionsPlugin from "wavesurfer.js/dist/plugins/regions.esm.js"`.
  `WaveSurfer.create({container, url?, height, waveColor, progressColor, cursorColor,
  barWidth, barGap, barRadius, minPxPerSec, normalize, dragToSeek, autoCenter, interact,
  peaks?, duration?, sampleRate?, media?})`. Events (exact arg tuples):
  `ready: [duration]`, `timeupdate: [t]`, `zoom: [minPxPerSec]`, `interaction: [newTime]`,
  `scroll: [visibleStartTime, visibleEndTime, scrollLeft, scrollRight]` (4 args in
  current 7.x). Methods: `zoom(pxPerSec)`, `setTime(t)`, `seekTo(progress01)`,
  `getDuration()`, `setScroll(px)`, `getScroll()`, `load(url, peaks?, duration?)`.
  RegionsPlugin: `create()` (no options), `addRegion({id?, start, end?, drag?, resize?,
  resizeStart?, resizeEnd?, color?, content?, minLength?, maxLength?})`, events
  `region-created`, `region-update: [region, side?]`, `region-updated: [region, side?]`,
  `region-clicked: [region, MouseEvent]`, `region-in/out`;
  `enableDragSelection(opts, threshold?) -> disposer`; `region.setOptions({start?, end?,
  color?, drag?, resize?, ...})`; `region.remove()`; `region.updatingSide` is public and
  set **only during user drags** (programmatic `setOptions` leaves it undefined).
  **Normative gotchas** (hyper-frames audio-editor CLAUDE.md + PRD): only
  `region-update`/`region-updated` exist (no `-start`/`-end` events); never `addRegion`
  before `ready`/non-zero duration (bounds clamp to 0 permanently); `addRegion` fires
  `region-created` synchronously — pre-tag own ids before calling; region content lives
  in a ShadowRoot — inject styles post-create via
  `ws.getWrapper().getRootNode()`; the band's `part` is multi-token — use
  `[part~="region"]`. wavesurfer fetches `url` with plain `fetch()` — works against the
  asset protocol because Tauri sets `Access-Control-Allow-Origin`.
- **zundo 2.3.0** (verified against npm + README + source, 2026-07-22; peerDep
  `zustand ^4.3 || ^5`): `import { temporal } from "zundo"`;
  `createStore<S>()(temporal(creator, options))`. Options: `limit`, `partialize:
  (state) => PartialState`, `equality: (past, current) => boolean` (true = skip),
  `handleSet`, `wrapTemporal`, **and init-time history seeding: `pastStates: [...],
  futureStates: [...]`** (documented; does not respect `limit`). Temporal API:
  `store.temporal.getState().{undo(steps?), redo(steps?), clear(), pause(), resume(),
  isTracking, pastStates, futureStates}`. `store.temporal` is a vanilla zustand store —
  React subscribes via `useStore(store.temporal, selector)`. `pause()` sets
  `isTracking: false`; sets made while paused record no history.
  **Coalescing caveat (design-critical):** zundo pushes the *previous* state on each
  tracked `set`. After paused interim sets, a plain resume-then-set would push the *last
  interim* state, not the pre-drag state. The correct drag pattern (Task 11) is: pause →
  interim sets → (still paused) restore pre-drag document → resume → set final document —
  exactly one history entry whose past state is pre-drag.
- **Tauri 2 asset protocol** (verified against v2.tauri.app config/security refs +
  tauri source, 2026-07-22): config `app.security.assetProtocol.{enable: bool, scope}`;
  scope is glob list or `{allow, deny}` with `$HOME`-style variables. Current
  `tauri.conf.json` has **no assetProtocol block and `"csp": null`** [P4-CONFIRM — the
  phase-4 implementer needs it for video playback and may have added it]. With `csp:
  null` no CSP directives are needed. `convertFileSrc` from `@tauri-apps/api/core`;
  macOS URL shape `asset://localhost/<encodeURIComponent(abs-path)>`. The protocol
  serves **HTTP Range** responses (206, ~1 MB chunks) — `<video>` seeking over large
  files works. Scope config in this plan: `"scope": ["$HOME/**", "/Volumes/**"]`
  (personal app; the studio root may live on either).
- **SRT / WebVTT** (WebVTT verified against w3.org/TR/webvtt1, 2026-07-22): VTT file =
  optional BOM + `WEBVTT` line + blank line; cue = optional identifier line + timing line
  `HH:MM:SS.mmm --> HH:MM:SS.mmm` (hours optional when 0 — we always emit them; `-->`
  space-surrounded; **period** before ms), payload lines, cues separated by ≥1 blank
  line; payload must not contain `-->` or consecutive blank lines; escape `&` → `&amp;`
  and `<` → `&lt;`. SRT (de-facto stable format): 1-based index line,
  `HH:MM:SS,mmm --> HH:MM:SS,mmm` (**comma** before ms), text, blank line between blocks.
- **ffmpeg render** (normative source: `hyper-frames/tools/cut-video` — math and graph
  text ported verbatim from `src/cut_video/segments.py`; encoder flags PRD-locked):
  per-keep `[0:v]trim=start=S:end=E,setpts=PTS-STARTPTS[v<i>]` +
  `[0:a]atrim=start=S:end=E,asetpts=PTS-STARTPTS[a<i>]`, single
  `[v0][a0]…concat=n=N:v=1:a=1[v][a]`, lines joined `";\n"` + trailing newline,
  boundaries formatted `%.6f`. Graph written to a **file** passed via
  `-filter_complex_script` (argv-limit safe). Encode: `-c:v libx264 -crf 18 -preset
  medium -pix_fmt yuv420p`, audio **aac** (PRD divergence from cut-video's pcm_s16le),
  `-movflags +faststart`, always re-encode, never `-c copy`. Because the atomic temp
  path ends in `.tmp`, pass `-f mp4` explicitly (ffmpeg infers muxer from extension).
- **quick-xml 0.41.0** (crates.io, 2026-07-22): `Writer::new(impl std::io::Write)`,
  `writer.create_element("tag").with_attribute(("k", "v")).write_empty()` /
  `.write_inner_content(|w| …)`, `writer.write_event(Event::Decl(BytesDecl::new("1.0",
  Some("UTF-8"), None)))`, `Event::DocType(BytesText::from_escaped("fcpxml"))` for the
  DOCTYPE. Attribute values are escaped by the library. **url 2.5.8**:
  `url::Url::from_file_path(abs_path) -> Result<Url, ()>` percent-encodes non-ASCII path
  segments — the `file://` source for `media-rep src`.
- **hyper-frames reuse map** (mirror read 2026-07-22): `tools/cut-video/segments.py` is
  the **verbatim** source for coalesce/keep-window/filtergraph semantics (Task 1) —
  divergences, all PRD-locked: katto computes in Rational and formats floats only into
  graph text; katto's audio codec is aac (PRD) not pcm/mp3; katto never uses
  `avg_frame_rate`. `tools/audio-editor/hooks/useCutsEditor.ts` is the normative
  drag-coalescing UX pattern (snapshot on drag start, single history entry on drag end,
  depth 100) — reimplemented on zundo, not ported. `tools/audio-editor` Waveform.tsx +
  its CLAUDE.md gotchas are normative for Task 16 (region sync loop with own-id tagging,
  isReady gate, shadow CSS injection, 5 ms skip-guard on timeupdate). The `.kruproj`
  history-in-edits.json mechanism has **no mirror antecedent** — defined here.
- **App-crate facts** (read from the current tree 2026-07-22): settings already expose
  `default_nle: Option<String>` in `Settings`/`SettingsPatch`
  (`src-tauri/src/commands/settings.rs`) backed by `db::settings::{get,set}` k/v — no
  migration needed. Jobs: `state.jobs.spawn(kind, label, payload_json, work) ->
  jobs_repo::Job` with `ctx.progress(value, message)` (`src-tauri/src/jobs.rs`).
  Keychain: `KeyService::{Elevenlabs, Anthropic}`. Opener plugin present:
  `tauri_plugin_opener::OpenerExt` with `reveal_item_in_dir` (used by
  `reveal_project_folder`, `src-tauri/src/commands/projects.rs:224`). Projects live at
  `<studio_root>/Projects/<slug>`; `db::events::record(conn, kind, project_slug,
  payload_json)`. App package name is `katto` (`cargo test -p katto`).

### Design grounding (Dribbble review 2026-07-22, filtered through the design system)

Shots reviewed: Eugene Dobrik "Video editor timeline" (16932506), Keltson Howell
"Transcript Editor UI" (8216074), Sabba Keynejad "Online Video Editor - Subtitle Tool"
(11034773), Victoria Grinevich "Video Editing App" (25425630), plus grid-level survey of
"audio waveform editor" / "video editing app dark".

**Taken:**
- Bottom timeline anatomy (Dobrik, Grinevich): a thin mono timecode **ruler** above the
  track; a **thumbnail filmstrip embedded in the track** itself; a 1 px playhead line
  with a small rectangular top handle; zoom control right-aligned in the transport row.
- Transport row **between video and timeline** (Grinevich): play/pause + mono
  `tabular-nums` `current / total` readout; one fixed 32 px row, ghost buttons.
- Karaoke-highlight of the currently-playing word in the transcript (Howell) as the
  transcript↔video sync signal — subtle `--surface-2` background on the active token,
  no color shift.
- Mono timecode gutter at paragraph starts (Keynejad's subtitle rows; already in the
  phase-4 transcript design — kept).
**Rejected (design-system filter):** numeric confidence readouts ("Confidence: 86.27%" —
banned numeric scoring); left tool-mode sidebars (katto has no tool modes); per-type
saturated clip-block colors (cuts are *dimmed/hatched absence*, not colored presence);
track-header column with lock/eye/mute (single track); "AI Actions" dropdown menus (AI
suggestions live inline, accepted one by one); purple/gradient accents, rounded-white
floating cards, glassmorphism.

---

## FCPXML 1.11 verified contract

Verified 2026-07-22 against the DTD Apple ships inside Final Cut Pro 11.2
(`/Applications/Final Cut Pro.app/Contents/Frameworks/Interchange.framework/Versions/A/Resources/FCPXMLv1_11.dtd`,
byte-identical to the CommandPost mirror), Apple's FCPXML Reference pages, a real FCP
1.11 export (fcp.cafe), and `xmllint --dtdvalid` runs over sample documents exercising
every construct below. Do not re-derive.

- **Skeleton:** `<?xml version="1.0" encoding="UTF-8"?>` + `<!DOCTYPE fcpxml>` (no
  SYSTEM id) + `<fcpxml version="1.11">`. Only `<resources>` is required; katto emits
  the safest shape: `resources` → `<event name="katto">` → `<project name="<slug>-v<N>">`
  → exactly one `<sequence>` → exactly one `<spine>`.
- **`<format>`** (EMPTY): `id` required; custom formats with explicit
  `frameDuration/width/height` and **no** `name` are fully valid (attributes override any
  predefined name). `frameDuration` is a `%time;` (e.g. `1001/30000s`); FCP's own exports
  use unreduced fractions. `colorSpace="1-1-1 (Rec. 709)"` (FCP reads only the triplet).
- **`<asset>`**: `id` required; `uid` **optional** (omitted → FCP creates a default
  clip; if ever supplied use reverse-DNS/UUID — uppercase-hex-only strings are reserved).
  Recommended attrs for offline-safety: `name, start="0s", duration, hasVideo="1",
  hasAudio="1", format (IDREF), audioSources="1", audioChannels="2", audioRate="48000"`
  (plain number here, unlike sequence). Child `<media-rep kind="original-media"
  src="file:///…"/>` is **required** in 1.11; `src` must be an absolute RFC-2396 URL —
  percent-encoded (use `url::Url::from_file_path`). Missing file imports *offline*
  (relinkable), not an error.
- **`<sequence>`**: `format` (IDREF) is the only required attr; katto emits `format,
  duration, tcStart="0s", tcFormat, audioLayout="stereo", audioRate="48k"` (enum here:
  `48k`, not `48000`). One mandatory `<spine>` child.
- **`<asset-clip>`**: `ref` (IDREF) required; attrs used: `name, offset, start,
  duration, enabled (0|1, default 1), lane, audioRole, format, tcFormat`. An asset-clip
  implicitly carries **all** media components of its asset (video + audio — no separate
  `<audio>` element; `audioRole="dialogue"`). Child order in its content model matters:
  note? → timing → intrinsic adjustments (`adjust-volume` etc.) → **anchored items** →
  markers → metadata.
- **Time values**: rational seconds `"N/Ds"` (64-bit num / 32-bit den) or whole `"5s"` /
  `"0s"`. Reduction NOT required; mixed denominators across one document are fine (FCP
  itself mixes `/2500`, `/10000`, `/720000`). **But**: sequence-timeline values that are
  not integer multiples of the sequence format's `frameDuration` make FCP insert a gap +
  warning on import — so the emitter snaps every boundary to the frame grid and writes
  all sequence-timeline times as `(frames × fps.den)/fps.num` + `"s"` in the format
  timebase. A decimal-seconds value anywhere is an emitter validation failure (PRD).
- **Rescue track (the load-bearing mechanism, DTD-verified):** a connected clip is an
  `asset-clip` **nested inside a spine clip's element** with `lane != 0` (`lane="-1"` =
  below the primary — katto's choice so re-enabled video never composites over the
  primary). `enabled="0"` is DTD-valid (`enabled (0 | 1) '1'`) and maps to FCP's
  Clip > Disable state — dimmed, excluded from playback/export, re-enabled with one
  keystroke (V). **Connected-clip `offset` is on the parent's LOCAL timeline whose
  origin is the parent's `start`**: `child_project_time = parent_offset + (child_offset
  − parent_start)`. Anchored items may extend past the parent's edges (parents do not
  clip anchored items). Rejected alternatives: a secondary storyline (`<spine lane=…>`
  nested) has no `enabled` attr and ripples internally; `adjust-volume amount="-96dB"`
  only silences audio and round-trips as *enabled* — optionally added to rescue clips as
  belt-and-suspenders, never as the mechanism.
- **`<gap>`**: `duration` required; valid spine item; **connected clips can anchor to a
  gap** (FCP's own exports do). Not needed by katto's emitter (keeps are contiguous) but
  the builder supports it.
- **Validation:** `xmllint --noout --dtdvalid "/Applications/Final Cut
  Pro.app/Contents/Frameworks/Interchange.framework/Versions/A/Resources/FCPXMLv1_11.dtd"
  file.fcpxml` — wired as an `#[ignore]`d test (runs only where FCP is installed).
  DTD-valid ≠ importable: bad timing/format data still fails at import; the manual FCP
  import stays an owner checkpoint.

---

## File Structure

```
crates/katto-engine/
  Cargo.toml                    # + quick-xml 0.41, url 2.5.8
  src/
    lib.rs                      # + pub mod emit, render, thumbs, timelines
    error.rs                    # + Render, Emit, Export, Relocate variants
    schema/edits.rs             # + EditSnapshot, EditHistory, Edits.history (Task 3)
    render.rs                   # render argv + progress parse + render_mp4 (Task 2)
    render/segments.rs          # keep-window math + filtergraph text, pure (Task 1)
    emit.rs                     # parent module: pub mod captions, fcpxml
    emit/captions.rs            # kept-only retiming + SRT + VTT emitters (Task 4)
    emit/fcpxml.rs              # emitter + pre-write validation (Task 6)
    emit/fcpxml/builder.rs      # typed element builder over quick-xml Writer (Task 5)
    thumbs.rs                   # 2s-cadence thumbnail extraction (Task 7)
    timelines.rs                # -vN allocation + export_timeline orchestration (Task 8)
  tests/fixtures/               # + cuts.500.json generator lives in tests, not fixtures

crates/katto-cli/
  src/cli.rs, main.rs, output.rs   # + Render/Export variants (Task 9)

src-tauri/
  tauri.conf.json               # assetProtocol enable + scope [P4-CONFIRM] (Task 10)
  src/commands/editor.rs        # save_edits, preview_export, export_timeline,
                                # render_mp4, generate_thumbs, relocate_source,
                                # open_in_fcp, reveal_timeline (Task 10)
  src/commands.rs, lib.rs       # register the domain (Task 10)

src/features/editor/
  model/wire.ts (+ .test.ts)          # seconds↔Rational conversion, doc↔Edits (Task 11)
  store/editor-store.ts (+ .test.ts)  # zundo temporal document store factory (Task 11)
  store/autosave.ts (+ .test.ts)      # 200ms debounce + retry/pause logic (Task 12)
  model/cut-ops.ts (+ .test.ts)       # toggle/apply/manual/adjust, pure (Task 13)
  model/snap.ts (+ .test.ts)          # token snap + outward snap, pure (Task 13)
  model/kept-ranges.ts (+ .test.ts)   # effective ranges, complement, seek-past (Task 13)
  transcript-pane.tsx (+ .test.tsx)   # + editing interactions (Task 14)
  transport.ts (+ .test.ts)           # pure key map (Task 15)
  use-transport.ts                    # video wiring: JKL, kept-only playback (Task 15)
  model/timeline-geometry.ts (+ .test.ts)  # time↔px, hit-tests, drag math (Task 16)
  timeline-pane.tsx                   # canvas track + ruler + interactions (Task 16)
  waveform.tsx                        # wavesurfer strip (Task 17)
  export-dialog.tsx (+ .test.tsx)     # formats, version preview, NLE target (Task 18)
  relocate-dialog.tsx                 # SourceMissing flow (Task 18)
  editor-view.tsx                     # 3-pane grid composition (Tasks 14-18 touch)
src/lib/ipc/editor.ts                 # typed wrappers (Task 11)

docs/overnight-run.md            # Phase 5 checkboxes (Task 19)
prd/index.md                     # status flip (Task 19)
```

Layout decision (design grounding → concrete): `editor-view.tsx` becomes a fixed grid
inside `[data-scroll-root]` — top region splits **transcript left (flexible, min 420 px)**
/ **video right (fixed 40% width, pinned)**; bottom strip full-width fixed height
(ruler 20 px + canvas track 64 px + waveform 72 px + transport row 32 px ≈ 188 px). The
transcript column is the only scroll region. No pane chrome beyond `--hairline`
separators; the window never scrolls.

---

### Task 1: Keep-window math + filtergraph text (`render/segments.rs`)

**Files:**
- Create: `crates/katto-engine/src/render.rs` (parent module stub: `pub mod segments;` +
  re-exports; the rest of the file lands in Task 2)
- Create: `crates/katto-engine/src/render/segments.rs`
- Modify: `crates/katto-engine/src/lib.rs` (`pub mod render;`), `error.rs`

**Interfaces:**
- Consumes: `Rational` [P4-CONFIRM: `new/checked_add/checked_sub/to_secs_f64/snap_to_frame/Ord`].
- Produces (verbatim-ported semantics from `hyper-frames/tools/cut-video/src/cut_video/segments.py`):

```rust
/// A kept (retained) window in source time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Keep { pub start: Rational, pub end: Rational }

/// Sort removed spans by start and merge overlapping/touching ones; drop empty spans.
pub fn coalesce_cuts(cuts: &[(Rational, Rational)]) -> Vec<(Rational, Rational)>;

/// Complement removed spans into kept windows over [0, duration]; drop keeps whose
/// duration <= one frame of `fps`; error when nothing survives.
///
/// # Errors
/// [`Error::WholeDurationRemoved`] when the cuts cover the entire source.
pub fn keep_windows(cuts: &[(Rational, Rational)], duration: Rational, fps: Rational)
    -> Result<Vec<Keep>>;

/// Deterministic ffmpeg filter_complex_script text for the keep list (both streams).
/// Boundaries formatted `%.6f`; byte-identical across runs for identical input.
pub fn filter_complex_script(keeps: &[Keep]) -> String;
```

Error variants added to `error.rs`:

```rust
    /// Every keep-window was removed or sub-epsilon; nothing remains to encode.
    #[error("whole duration removed: the cuts cover the entire source")]
    WholeDurationRemoved,
    /// MP4 render failed (ffmpeg stderr tail included).
    #[error("render: {0}")]
    Render(String),
```

Semantics (each maps 1:1 to segments.py — parity cases below quote its behavior):
coalesce = sort by start, merge when `start <= prev_end` (touching merges), keep
`max(prev_end, end)`, drop `end <= start` spans. keep_windows = clip cuts to
`[0, duration]`, cursor-walk from 0 emitting `[cursor, cut_start]`, final
`[last_end, duration]`, epsilon = 1 frame (`fps.den/fps.num` seconds — compare with
`Rational` arithmetic: keep survives iff `end − start > epsilon`). Graph text = per-keep
`[0:v]trim=start=S:end=E,setpts=PTS-STARTPTS[v<i>]` and
`[0:a]atrim=start=S:end=E,asetpts=PTS-STARTPTS[a<i>]`, then
`[v0][a0][v1][a1]…concat=n=<N>:v=1:a=1[v][a]`, joined `";\n"` + trailing `"\n"`.
`S`/`E` from `format!("{:.6}", keep.start.to_secs_f64())` (matches Python `f"{x:.6f}"`).
No `snap` parameter (katto snaps at a higher layer); duration <= 0 → `WholeDurationRemoved`.

- [ ] **Step 1: Write the failing tests** (in `segments.rs` `mod tests`; `TB`-style
  helpers; parity cases derived from segments.py):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const TB: u32 = 1000;
    fn r(ms: i64) -> Rational { Rational::new(ms, TB) }
    const FPS25: Rational = Rational { num: 25, den: 1 };

    #[test]
    fn coalesce_merges_touching_and_overlapping() {
        // parity: segments.py merges start <= prev_end (touching included)
        let out = coalesce_cuts(&[(r(2000), r(3000)), (r(1000), r(2000)), (r(2500), r(2600))]);
        assert_eq!(out, vec![(r(1000), r(3000))]);
    }

    #[test]
    fn coalesce_drops_empty_spans() {
        assert_eq!(coalesce_cuts(&[(r(1000), r(1000)), (r(2000), r(1500))]), vec![]);
    }

    #[test]
    fn keep_windows_walks_cursor_and_emits_tail() {
        let keeps = keep_windows(&[(r(1000), r(2000))], r(5000), FPS25).unwrap();
        assert_eq!(keeps, vec![
            Keep { start: r(0), end: r(1000) },
            Keep { start: r(2000), end: r(5000) },
        ]);
    }

    #[test]
    fn keep_windows_clips_out_of_range_cuts() {
        // parity: clipped_start = max(cut_start, 0), clipped_end = min(cut_end, duration)
        let keeps = keep_windows(&[(r(-500), r(1000)), (r(4500), r(9000))], r(5000), FPS25).unwrap();
        assert_eq!(keeps, vec![Keep { start: r(1000), end: r(4500) }]);
    }

    #[test]
    fn sub_epsilon_keep_between_cuts_is_dropped() {
        // 25fps epsilon = 40ms; the only surviving window is the 30ms sliver between
        // the two cuts -> dropped -> nothing remains -> loud error (segments.py parity)
        assert!(matches!(
            keep_windows(&[(r(0), r(1000)), (r(1030), r(5000))], r(5000), FPS25),
            Err(Error::WholeDurationRemoved)
        ));
        // and a sliver bigger than epsilon survives
        let keeps = keep_windows(&[(r(0), r(1000)), (r(1050), r(5000))], r(5000), FPS25).unwrap();
        assert_eq!(keeps, vec![Keep { start: r(1000), end: r(1050) }]);
    }

    #[test]
    fn whole_duration_removed_is_loud() {
        assert!(matches!(
            keep_windows(&[(r(0), r(5000))], r(5000), FPS25),
            Err(Error::WholeDurationRemoved)
        ));
    }

    #[test]
    fn filtergraph_snapshot_basic() {
        let keeps = keep_windows(&[(r(1000), r(2000))], r(5000), FPS25).unwrap();
        insta::assert_snapshot!("filtergraph_basic", filter_complex_script(&keeps));
    }

    #[test]
    fn filtergraph_500_cuts_is_structurally_sound() {
        // argv-limit-safety fixture: 600 alternating cuts over a long source
        let cuts: Vec<_> = (0..600)
            .map(|i| (r(i * 2000 + 1000), r(i * 2000 + 1500)))
            .collect();
        let keeps = keep_windows(&cuts, r(600 * 2000 + 1000), FPS25).unwrap();
        assert_eq!(keeps.len(), 601);
        let graph = filter_complex_script(&keeps);
        let lines: Vec<_> = graph.trim_end().split(";\n").collect();
        assert_eq!(lines.len(), 2 * 601 + 1);
        assert!(lines.last().unwrap().contains("concat=n=601:v=1:a=1[v][a]"));
        assert!(graph.ends_with('\n'));
    }

    #[test]
    fn filtergraph_is_byte_deterministic() {
        let keeps = keep_windows(&[(r(1234), r(4567))], r(10000), FPS25).unwrap();
        assert_eq!(filter_complex_script(&keeps), filter_complex_script(&keeps));
        assert!(filter_complex_script(&keeps).contains("trim=start=0.000000:end=1.234000"));
    }
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p katto-engine segments` →
  compile errors (module missing).

- [ ] **Step 3: Implement** per the semantics block. Comparisons and epsilon math use
  `Rational` `Ord`/`checked_sub` (no floats); only `filter_complex_script` projects to
  `f64` for formatting. ~90 lines.

- [ ] **Step 4: Run** — `cargo test -p katto-engine segments` → PASS; review + commit the
  new `.snap` (never `.snap.new`); the `emitter-snapshot-change` skill governs this file
  from now on.

- [ ] **Step 5: Commit**

```bash
git add crates/katto-engine/src/render.rs crates/katto-engine/src/render/ \
        crates/katto-engine/src/lib.rs crates/katto-engine/src/error.rs \
        crates/katto-engine/src/render/snapshots/
git commit -m "feat(engine): keep-window math and deterministic filtergraph text"
```

---

### Task 2: MP4 render — argv, progress parse, orchestration (`render.rs`)

**Files:**
- Modify: `crates/katto-engine/src/render.rs`

**Interfaces:**
- Consumes: Task 1; `bundle::Bundle` + `merge::effective_cuts` [P4-CONFIRM: `CutPlan::from_wire(cuts, timebase)`, `effective_cuts(&plan, &edits) -> Vec<EffectiveCut>`], `ProjectManifest {frame_rate, duration, source_video_absolute_path}`.
- Produces:

```rust
/// Pinned ffmpeg argv for the kept-only re-encode (never -c copy; graph via script file).
pub fn render_argv(src: &Path, graph_path: &Path, out_tmp: &Path) -> Vec<String>;
// ffmpeg -nostdin -loglevel error -y -progress pipe:1 -i <src>
//   -filter_complex_script <graph_path> -map [v] -map [a]
//   -c:v libx264 -crf 18 -preset medium -pix_fmt yuv420p
//   -c:a aac -b:a 192k -movflags +faststart -f mp4 <out_tmp>

/// Parse one `-progress pipe:1` line; `out_time_us=N` -> seconds rendered so far.
pub fn parse_progress_line(line: &str) -> Option<f64>;

/// Kept-source seconds total (progress denominator).
pub fn kept_total_secs(keeps: &[Keep]) -> f64;

/// Effective cuts (frame-snapped, coalesced) -> keeps -> graph file -> ffmpeg -> atomic
/// rename to `out`. `on_progress` receives 0.0..=1.0.
///
/// # Errors
/// [`Error::WholeDurationRemoved`] before any spawn; [`Error::Render`] with the stderr
/// tail on ffmpeg failure (partial `.tmp` removed; bundle untouched).
pub async fn render_mp4(
    bundle: &Bundle,
    out: &Path,
    on_progress: &(dyn Fn(f64) + Send + Sync),
) -> Result<()>;

/// Shared by render and the FCPXML emitter: effective cuts snapped to the frame grid
/// and coalesced, in the manifest's frame timebase (den = fps.num).
pub fn effective_cut_spans(bundle: &Bundle) -> Result<Vec<(Rational, Rational)>>;
```

`effective_cut_spans`: requires `bundle.cuts` (else `Error::Bundle("no cuts.json yet")`);
`timebase = frame_rate.num as u32` (frame-exact: frame k = `k*fps.den/fps.num` s =
`(k*fps.den)/timebase`); `CutPlan::from_wire(cuts, timebase)` + `edits` (default when
absent, **history stripped before merge is irrelevant — merge ignores unknown fields**)
→ `effective_cuts` → each span `snap_to_frame(fps)` on both edges → drop inverted →
return spans (coalescing happens inside `keep_windows`). Graph file: write
`filter_complex_script` text to `<bundle>/render.filtergraph` via `bundle::write_atomic`
(regenerated every render; not an export artifact). Spawn: `tokio::process::Command`,
stdout line-reader feeding `parse_progress_line` → `on_progress(min(secs/total, 1.0))`,
stderr captured for the error tail; on non-zero exit remove `<out>.tmp`.

- [ ] **Step 1: Write the failing tests** (in `render.rs` `mod tests`):

```rust
#[test]
fn render_argv_is_pinned_and_tmp_safe() {
    let argv = render_argv(Path::new("/a/clip.mp4"), Path::new("/b/g.txt"), Path::new("/c/out.mp4.tmp"));
    assert_eq!(argv, vec![
        "ffmpeg", "-nostdin", "-loglevel", "error", "-y", "-progress", "pipe:1",
        "-i", "/a/clip.mp4",
        "-filter_complex_script", "/b/g.txt",
        "-map", "[v]", "-map", "[a]",
        "-c:v", "libx264", "-crf", "18", "-preset", "medium", "-pix_fmt", "yuv420p",
        "-c:a", "aac", "-b:a", "192k",
        "-movflags", "+faststart", "-f", "mp4",
        "/c/out.mp4.tmp",
    ]);
}

#[test]
fn progress_line_parses_out_time_us() {
    assert_eq!(parse_progress_line("out_time_us=1500000"), Some(1.5));
    assert_eq!(parse_progress_line("frame=42"), None);
    assert_eq!(parse_progress_line("out_time_us=N/A"), None);
}

#[test]
fn effective_cut_spans_snaps_and_merges() {
    // manifest at 25fps; a wire cut at 1.013..2.017 snaps to 1.00..2.00 (nearest frame)
    let bundle = test_bundle_with_cuts(&[(1.013, 2.017)]); // helper: in-memory Bundle
    let spans = effective_cut_spans(&bundle).unwrap();
    assert_eq!(spans, vec![(Rational::new(25, 25), Rational::new(50, 25))]);
}
```

  (`test_bundle_with_cuts` builds a `Bundle` literal: manifest `frame_rate 25/1`,
  `duration 10s` (`Rational::new(250, 25)`), cuts from the tuple list with
  `CutReason::Filler`, `edits: None`, `transcript: None`, `root: PathBuf::new()`,
  source path empty — a pure-value helper, no filesystem.)

  Plus the ignored real-ffmpeg test:

```rust
#[tokio::test]
#[ignore = "spawns real ffmpeg; owner checkpoint (KATTO_TEST_CLIP)"]
async fn render_real_clip_end_to_end() { /* import-style guard on env var; render to tempdir; assert out exists, no .tmp litter */ }
```

- [ ] **Step 2: Run to verify failure**, **Step 3: Implement**, **Step 4: Run** —
  `cargo test -p katto-engine render` → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/katto-engine/src/render.rs
git commit -m "feat(engine): kept-only MP4 render with pinned encode and streamed progress"
```

---

### Task 3: Persistent-undo wire format (`schema/edits.rs` history)

**Files:**
- Modify: `crates/katto-engine/src/schema/edits.rs`

**Interfaces:**
- Consumes: phase-4 `Edits`/`ManualCut`/`BoundaryAdjustment` [P4-CONFIRM exact field
  names — the plan-of-record shape is `Edits {schema_version, toggled_off,
  applied_discretionary, manual_cuts, boundary_adjustments}`].
- Produces (PRD: "history persisted in `edits.json`, survives restart", depth 100):

```rust
/// One undo/redo step: the document-state fields of [`Edits`] at a point in time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EditSnapshot {
    #[serde(default)] pub toggled_off: Vec<usize>,
    #[serde(default)] pub applied_discretionary: Vec<usize>,
    #[serde(default)] pub manual_cuts: Vec<ManualCut>,
    #[serde(default)] pub boundary_adjustments: Vec<BoundaryAdjustment>,
}

/// Undo/redo stacks persisted across sessions (bounded to 100 by the frontend).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EditHistory {
    #[serde(default)] pub past: Vec<EditSnapshot>,
    #[serde(default)] pub future: Vec<EditSnapshot>,
}

// Edits gains one field (phase-4 files parse unchanged; merge ignores it):
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<EditHistory>,
```

- [ ] **Step 1: Write the failing tests** (append to `edits.rs` `mod tests`):

```rust
#[test]
fn edits_without_history_still_parses() {
    let e: Edits = serde_json::from_str(r#"{"schema_version":1,"toggled_off":[1]}"#).unwrap();
    assert!(e.history.is_none());
}

#[test]
fn history_round_trips_and_is_omitted_when_none() {
    let e = Edits {
        schema_version: 1,
        history: Some(EditHistory {
            past: vec![EditSnapshot { toggled_off: vec![0], ..Default::default() }],
            future: vec![],
        }),
        ..Default::default()
    };
    let json = serde_json::to_string(&e).unwrap();
    assert_eq!(serde_json::from_str::<Edits>(&json).unwrap(), e);
    let bare = serde_json::to_string(&Edits::default()).unwrap();
    assert!(!bare.contains("history"));
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p katto-engine edits`.
- [ ] **Step 3: Implement** (struct additions above; `///` docs; fix any struct-literal
  call sites the compiler flags — `merge.rs` tests use `..Default::default()` and stay
  untouched).
- [ ] **Step 4: Run** — `cargo test -p katto-engine` → PASS (whole crate; merge must not
  regress).
- [ ] **Step 5: Commit**

```bash
git add crates/katto-engine/src/schema/edits.rs
git commit -m "feat(engine): persist undo history in the edits wire format"
```

---

### Task 4: Kept-only caption retiming + SRT/VTT emitters (`emit/captions.rs`)

**Files:**
- Create: `crates/katto-engine/src/emit.rs` (`pub mod captions;` — `pub mod fcpxml;`
  added in Task 5-6)
- Create: `crates/katto-engine/src/emit/captions.rs`
- Modify: `crates/katto-engine/src/lib.rs` (`pub mod emit;`)

**Interfaces:**
- Consumes: `schema::{Transcript, WordEntry}` [P4-CONFIRM variant accessors
  `start()/end()/text()`], Task 2's `effective_cut_spans` output shape
  `&[(Rational, Rational)]`.
- Produces:

```rust
/// A caption cue in output (kept-only) time.
#[derive(Debug, Clone, PartialEq)]
pub struct Caption { pub start: Rational, pub end: Rational, pub text: String }

/// Drop words inside cuts; shift survivors left by the preceding removed total.
/// Cuts must be sorted+disjoint (the caller passes coalesced spans). Word containment
/// is by midpoint (cut boundaries sit on token edges per validation invariant 7).
pub fn retime_kept_words(words: &[WordEntry], cuts: &[(Rational, Rational)], timebase: u32)
    -> Vec<Caption>;   // one Caption per kept Word token (spacing/audio_event dropped)

/// Group per-word cues into caption lines: break at sentence end (./?/!), or when a
/// line would exceed 42 chars, or at a >1.0s kept-time gap.
pub fn group_captions(words: &[Caption]) -> Vec<Caption>;

/// SRT text: 1-based index, `HH:MM:SS,mmm --> HH:MM:SS,mmm`, blank-line separated.
pub fn emit_srt(captions: &[Caption]) -> String;

/// WebVTT text: `WEBVTT` + blank line; `HH:MM:SS.mmm` timestamps; `&`->`&amp;` `<`->`&lt;`.
pub fn emit_vtt(captions: &[Caption]) -> String;
```

Retiming math (all Rational): running `removed_before` accumulates cut durations whose
`end <= word_mid`; kept word emits `start − removed_before` / `end − removed_before`
(clamped ≥ 0). Timestamp rendering: total ms = `round(t.to_secs_f64() * 1000)` — the only
float projection, at the text boundary; hours always emitted (`00:` fine in both formats).

- [ ] **Step 1: Write the failing tests** (in `captions.rs` `mod tests`):

```rust
const TB: u32 = 1000;
fn r(ms: i64) -> Rational { Rational::new(ms, TB) }
fn w(text: &str, s: f64, e: f64) -> WordEntry {
    WordEntry::Word { text: text.into(), start: s, end: e, logprob: None, speaker_id: None }
}

#[test]
fn words_inside_cuts_are_dropped_and_survivors_shift_left() {
    let words = vec![w("keep", 0.0, 0.5), w("cut", 1.0, 1.5), w("tail", 2.0, 2.5)];
    let out = retime_kept_words(&words, &[(r(1000), r(2000))], TB);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].text, "keep");
    assert_eq!(out[1].start, r(1000));  // 2.0 - 1.0s removed
}

#[test]
fn grouping_breaks_at_sentence_end_and_42_chars() {
    let words: Vec<Caption> = ["This", "is", "a", "sentence."].iter().enumerate()
        .map(|(i, t)| Caption { start: r(i as i64 * 300), end: r(i as i64 * 300 + 250), text: (*t).into() })
        .chain([Caption { start: r(1500), end: r(1800), text: "Next".into() }])
        .collect();
    let lines = group_captions(&words);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].text, "This is a sentence.");
    assert_eq!(lines[0].end, r(1150));
}

#[test]
fn srt_and_vtt_snapshots() {
    let caps = vec![
        Caption { start: r(0), end: r(1200), text: "Hello <world> & co".into() },
        Caption { start: r(3600_000), end: r(3601_500), text: "Past the hour".into() },
    ];
    insta::assert_snapshot!("captions_srt", emit_srt(&caps));
    insta::assert_snapshot!("captions_vtt", emit_vtt(&caps));
}
```

  (Expected snapshot content — eyeball at review time: SRT block 1
  `00:00:00,000 --> 00:00:01,200` with raw `Hello <world> & co`; VTT header `WEBVTT`,
  `00:00:00.000 --> 00:00:01.200`, escaped `Hello &lt;world> &amp; co`; second cue
  `01:00:00,000` / `01:00:00.000`.)

- [ ] **Step 2: Run to verify failure** — `cargo test -p katto-engine captions`.
- [ ] **Step 3: Implement** (pure; no I/O).
- [ ] **Step 4: Run** — PASS; review + commit `.snap` files (emitter-snapshot-change
  discipline applies from now on).
- [ ] **Step 5: Commit**

```bash
git add crates/katto-engine/src/emit.rs crates/katto-engine/src/emit/captions.rs \
        crates/katto-engine/src/lib.rs crates/katto-engine/src/emit/snapshots/
git commit -m "feat(engine): kept-only caption retiming with SRT and VTT emitters"
```

---

### Task 5: FCPXML typed builder (`emit/fcpxml/builder.rs`)

**Files:**
- Create: `crates/katto-engine/src/emit/fcpxml.rs` (parent stub: `pub mod builder;` +
  re-exports; emitter body lands in Task 6)
- Create: `crates/katto-engine/src/emit/fcpxml/builder.rs`
- Modify: `crates/katto-engine/src/emit.rs` (`pub mod fcpxml;`),
  `crates/katto-engine/Cargo.toml` (`quick-xml = "0.41"`, `url = "2.5"`)

**Interfaces:**
- Consumes: `Rational`; the FCPXML contract section above (normative).
- Produces (a small typed layer so the Task-6 emitter can't emit malformed structure —
  serde fights the heterogeneous spine, hence hand-built, PRD-locked):

```rust
/// Render a Rational as an FCPXML time attribute: `"0s"` for zero, whole `"<n>s"` when
/// den divides num evenly, else `"<num>/<den>s"`. Never a decimal.
pub fn time_attr(t: Rational) -> String;

/// One rescue (removed) segment connected below a spine clip.
pub struct RescueClip {
    pub name: String,
    pub source_start: Rational,   // in-point in the asset
    pub duration: Rational,
    pub offset: Rational,         // on the PARENT's local timeline (origin = parent start)
}

/// One kept segment on the primary spine, carrying its anchored rescue clips.
pub struct SpineClip {
    pub name: String,
    pub offset: Rational,         // sequence time
    pub source_start: Rational,   // in-point in the asset (also the local-timeline origin)
    pub duration: Rational,
    pub rescues: Vec<RescueClip>,
}

/// Everything the document needs; times already frame-aligned by the caller.
pub struct FcpxmlDoc {
    pub event_name: String,           // "katto"
    pub project_name: String,         // "<slug>-v<N>"
    pub format_id: String,            // "r1"
    pub asset_id: String,             // "r2"
    pub frame_duration: Rational,     // fps.den/fps.num
    pub width: u32, pub height: u32,
    pub tc_format_df: bool,           // DF for 29.97/59.94, else NDF
    pub asset_name: String,
    pub asset_duration: Rational,
    pub media_src: String,            // percent-encoded file:/// URL
    pub sequence_duration: Rational,
    pub clips: Vec<SpineClip>,
}

/// Serialize the whole document (XML decl + DOCTYPE + fcpxml tree). Pure.
pub fn write_document(doc: &FcpxmlDoc) -> String;
```

Emission rules (each from the verified contract): skeleton `resources → event → project
→ sequence → spine`; `<format id name=∅ frameDuration width height
colorSpace="1-1-1 (Rec. 709)"/>`; `<asset id name uid=∅ start="0s" duration hasVideo="1"
hasAudio="1" format audioSources="1" audioChannels="2" audioRate="48000">` with child
`<media-rep kind="original-media" src=…/>`; `<sequence format duration tcStart="0s"
tcFormat audioLayout="stereo" audioRate="48k"><spine>…`; kept clip `<asset-clip ref
offset name start duration audioRole="dialogue">` containing each rescue as
`<asset-clip ref lane="-1" offset name start duration enabled="0"
audioRole="dialogue"/>`. Attribute order fixed as listed (byte-stable snapshots).
Two-space indentation, `\n` line ends.

- [ ] **Step 1: Write the failing tests** (in `builder.rs` `mod tests`):

```rust
#[test]
fn time_attr_forms() {
    assert_eq!(time_attr(Rational::new(0, 30000)), "0s");
    assert_eq!(time_attr(Rational::new(60000, 30000)), "2s");
    assert_eq!(time_attr(Rational::new(1001, 30000)), "1001/30000s");
}

fn tiny_doc() -> FcpxmlDoc {
    FcpxmlDoc {
        event_name: "katto".into(), project_name: "demo-v1".into(),
        format_id: "r1".into(), asset_id: "r2".into(),
        frame_duration: Rational::new(1, 25), width: 3840, height: 2160,
        tc_format_df: false,
        asset_name: "clip.mp4".into(), asset_duration: Rational::new(250, 25),
        media_src: "file:///a/clip.mp4".into(),
        sequence_duration: Rational::new(225, 25),
        clips: vec![SpineClip {
            name: "keep 1".into(), offset: Rational::new(0, 25),
            source_start: Rational::new(0, 25), duration: Rational::new(225, 25),
            rescues: vec![RescueClip {
                name: "removed 1".into(), source_start: Rational::new(100, 25),
                duration: Rational::new(25, 25), offset: Rational::new(100, 25),
            }],
        }],
    }
}

#[test]
fn document_snapshot_tiny() {
    insta::assert_snapshot!("fcpxml_builder_tiny", write_document(&tiny_doc()));
}

#[test]
fn rescue_clips_are_disabled_on_lane_minus_one() {
    let xml = write_document(&tiny_doc());
    assert!(xml.contains(r#"lane="-1""#));
    assert!(xml.contains(r#"enabled="0""#));
    assert!(xml.contains("<!DOCTYPE fcpxml>"));
    assert!(xml.contains(r#"<fcpxml version="1.11">"#));
    assert!(!xml.contains('.') || !xml.contains(r#"duration="9.0"#), "no decimal times");
}

#[test]
fn xml_special_chars_in_names_are_escaped() {
    let mut doc = tiny_doc();
    doc.asset_name = r#"a<b&"c".mp4"#.into();
    let xml = write_document(&doc);
    assert!(xml.contains("a&lt;b&amp;"));
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p katto-engine builder`.
- [ ] **Step 3: Implement** with `quick_xml::Writer::new_with_indent(&mut buf, b' ', 2)`;
  events: `BytesDecl::new("1.0", Some("UTF-8"), None)`,
  `Event::DocType(BytesText::from_escaped("fcpxml"))`, then `create_element` chains.
  (`0.41` API: if `new_with_indent`'s signature differs, fall back to
  `Writer::new(&mut buf)` + manual indent — check the installed docs.rs page once; the
  snapshot freezes whatever is produced.)
- [ ] **Step 4: Run** — PASS; eyeball the snapshot against the contract section (order,
  attributes, DOCTYPE), commit `.snap`.
- [ ] **Step 5: Commit**

```bash
git add crates/katto-engine/src/emit/fcpxml.rs crates/katto-engine/src/emit/fcpxml/ \
        crates/katto-engine/src/emit.rs crates/katto-engine/Cargo.toml Cargo.lock
git commit -m "feat(engine): typed FCPXML 1.11 document builder"
```

---

### Task 6: FCPXML emitter with rescue track (`emit/fcpxml.rs`)

**Files:**
- Modify: `crates/katto-engine/src/emit/fcpxml.rs`
- Modify: `crates/katto-engine/src/error.rs`

**Interfaces:**
- Consumes: Task 5 builder; Task 1 `keep_windows`/`Keep`; Task 2 `effective_cut_spans`;
  `ProjectManifest` [P4-CONFIRM: `frame_rate`, `duration`,
  `source_video_absolute_path`]. Width/height: probe data is not in the manifest —
  emit the constant 3840×2160? **No** — width/height affect only the format resource;
  FCP derives real dimensions from the media. Decision: emit the format WITHOUT
  width/height (all format attrs are `#IMPLIED`; frameDuration is what matters), so no
  probe dependency. Documented in the emitter's module doc.
- Produces:

```rust
/// Named invariant violations checked before any write (PRD: "a decimal second
/// anywhere is a validation failure").
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FcpxmlInvariant {
    #[error("clip {index}: {attr} {value:?} is not on the frame grid (frameDuration {frame_duration:?})")]
    OffFrameGrid { index: usize, attr: &'static str, value: Rational, frame_duration: Rational },
    #[error("clips are not contiguous at index {index}: expected offset {expected:?}, got {got:?}")]
    NotContiguous { index: usize, expected: Rational, got: Rational },
    #[error("rescue {index} of clip {clip}: zero or negative duration")]
    EmptyRescue { index: usize, clip: usize },
}

/// Build the document model from a bundle's effective state. Pure.
///
/// # Errors
/// [`Error::WholeDurationRemoved`]; [`Error::Bundle`] when cuts.json is absent.
pub fn build_document(bundle: &Bundle, project_name: &str) -> Result<FcpxmlDoc>;

/// Validate every emitted time attribute against the frame grid + spine contiguity.
pub fn validate_document(doc: &FcpxmlDoc) -> Vec<FcpxmlInvariant>;

/// build + validate + serialize; validation failure aborts before any caller write.
pub fn emit_fcpxml(bundle: &Bundle, project_name: &str) -> Result<String>;
```

Construction rules: `fps = manifest.frame_rate`; spans = `effective_cut_spans` →
`keep_windows(spans, duration, fps)` → coalesced removed spans =
`coalesce_cuts(spans)` clipped to `[0, duration]`. Spine clips: keep i → `SpineClip
{name: format!("keep {}", i+1), offset: running kept sum, source_start: keep.start,
duration: keep.end − keep.start}`. Rescue attachment: removed span j anchors to the
kept clip **preceding it** (the keep whose `end == removed.start` after coalescing; a
removed span at t=0 anchors to the first keep). Child offset on the parent's local
timeline: `offset = parent.source_start + parent.duration` for a following rescue
(places it exactly at the cut point in project time: `parent_offset + (child_offset −
parent_start) = parent_offset + parent_duration`); for the t=0 rescue, `offset =
parent.source_start − removed.duration` (projects before sequence time 0 — anchored
items may extend past parent edges per the contract; if the Task-6 DTD gate or the
owner's FCP import shows FCP mishandling a negative project position, the checked
fallback is `offset = parent.source_start` — rescue sits at project 0 overlapping the
first keep on the lower disabled lane, still one V-press to recover). Rescue
`source_start = removed.start`, `duration = removed span length`, `name: format!("removed
{}", j+1)`. `tc_format_df = fps.den == 1001 && (fps.num == 30000 || fps.num == 60000)`.
`sequence_duration` = Σ keep durations. `media_src =
url::Url::from_file_path(&manifest.source_video_absolute_path)` (non-absolute path →
`Error::Bundle`). All times constructed in the frame timebase (den = fps.num as u32);
`validate_document` re-checks each against `frame_duration` divisibility (belt and
braces — a future edit can't sneak a decimal in) and spine contiguity.

- [ ] **Step 1: Write the failing tests** (in `fcpxml.rs` `mod tests`; reuse Task 2's
  pure `test_bundle_with_cuts` pattern — hoist it into a `#[cfg(test)] pub(crate) mod
  test_support` in `render.rs` in this step; **fixture set is the PRD's normative list**):

```rust
// helpers: bundle_25fps(cuts: &[(f64, f64)]) via test_support; NTSC via frame_rate 30000/1001.

#[test]
fn snapshot_basic_two_cuts() {
    let b = bundle_25fps(&[(1.0, 2.0), (4.0, 5.0)]);   // duration 10s
    insta::assert_snapshot!("fcpxml_basic", emit_fcpxml(&b, "demo-v1").unwrap());
}

#[test]
fn snapshot_rescue_track_every_removed_segment_present() {
    let b = bundle_25fps(&[(0.0, 1.0), (5.0, 6.0)]);   // includes a t=0 cut
    let xml = emit_fcpxml(&b, "demo-v1").unwrap();
    assert_eq!(xml.matches(r#"enabled="0""#).count(), 2);   // one rescue per removed span
    insta::assert_snapshot!("fcpxml_rescue", xml);
}

#[test]
fn snapshot_ntsc_df() {
    let b = bundle_ntsc(&[(1.0, 2.0)]);                // 30000/1001
    let xml = emit_fcpxml(&b, "demo-v1").unwrap();
    assert!(xml.contains(r#"tcFormat="DF""#));
    assert!(xml.contains("1001/30000s") || xml.contains("/30000s"));
    insta::assert_snapshot!("fcpxml_ntsc_df", xml);
}

#[test]
fn unicode_filename_is_percent_encoded() {
    let b = bundle_named_25fps("clip – プレビュー.mp4", &[(1.0, 2.0)]);
    let xml = emit_fcpxml(&b, "demo-v1").unwrap();
    assert!(xml.contains("file:///"));
    assert!(!xml.contains("プレビュー"), "raw non-ASCII must not appear in media_src");
    insta::assert_snapshot!("fcpxml_unicode_src", xml);
}

#[test]
fn five_hundred_cuts_document_is_valid_and_bounded() {
    let cuts: Vec<(f64, f64)> = (0..500).map(|i| (i as f64 * 4.0 + 1.0, i as f64 * 4.0 + 1.5)).collect();
    let b = bundle_25fps_with_duration(&cuts, 2001.0);
    let xml = emit_fcpxml(&b, "demo-v1").unwrap();
    assert_eq!(xml.matches("<asset-clip").count(), 500 + 501);  // rescues + keeps
}

#[test]
fn off_grid_time_is_a_named_validation_failure() {
    let mut doc = /* build_document(...) on a valid bundle */;
    doc.clips[0].duration = Rational::new(3, 1000);  // not a 25fps frame multiple
    let errs = validate_document(&doc);
    assert!(errs.iter().any(|e| matches!(e, FcpxmlInvariant::OffFrameGrid { attr: "duration", .. })));
}
```

  Plus the DTD gate (owner machine has FCP):

```rust
#[test]
#[ignore = "requires Final Cut Pro's shipped DTD + xmllint; owner checkpoint"]
fn dtd_validates_against_fcpxml_v1_11() {
    // write emit_fcpxml output to a temp file; std::process::Command xmllint --noout
    // --dtdvalid "/Applications/Final Cut Pro.app/Contents/Frameworks/Interchange.framework/Versions/A/Resources/FCPXMLv1_11.dtd"
    // assert exit success, else print stderr
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p katto-engine fcpxml`.
- [ ] **Step 3: Implement** per construction rules. Error enum gains
  `#[error("fcpxml invariant: {0}")] Fcpxml(String)` (joined invariant Displays) — the
  export layer surfaces it verbatim ("export aborted before any file write").
- [ ] **Step 4: Run** — PASS. Eyeball each snapshot against the contract (spine
  contiguity, rescue lane/enabled, DF flag, percent-encoding), commit `.snap` files.
  Run the ignored DTD test once now (`cargo test -p katto-engine fcpxml -- --ignored`)
  — FCP is installed on this machine; fix anything xmllint rejects before moving on.
- [ ] **Step 5: Commit**

```bash
git add crates/katto-engine/src/emit/fcpxml.rs crates/katto-engine/src/error.rs \
        crates/katto-engine/src/render.rs crates/katto-engine/src/emit/snapshots/
git commit -m "feat(engine): FCPXML 1.11 emitter with disabled-lane rescue track"
```

---

### Task 7: Timeline thumbnails (`thumbs.rs`)

**Files:**
- Create: `crates/katto-engine/src/thumbs.rs`
- Modify: `crates/katto-engine/src/lib.rs`

**Interfaces:**
- Consumes: `bundle::Bundle`; tokio process (same thin-spawn pattern as import).
- Produces:

```rust
/// Bundle-relative thumbnails directory name.
pub const THUMBS_DIR: &str = "thumbs";

/// Pinned argv: one 320px-wide JPEG every 2 seconds into `<out_dir>/%05d.jpg`.
pub fn thumbs_argv(src: &Path, out_dir: &Path) -> Vec<String>;
// ffmpeg -nostdin -loglevel error -y -i <src> -vf fps=1/2,scale=320:-2 -q:v 5 <out_dir>/%05d.jpg

/// Regenerate `<bundle>/thumbs/` atomically (render into `thumbs.tmp/`, swap dirs).
/// Returns the frame count. Idempotent.
pub async fn generate_thumbs(bundle_root: &Path, src: &Path) -> Result<u32>;
```

Swap semantics: extract into `<bundle>/thumbs.tmp/` (created fresh, removed first if
lingering), then `remove_dir_all(thumbs)` (ignore missing) + `rename(thumbs.tmp,
thumbs)`. Count = files matching `*.jpg` after rename. ffmpeg failure → `Error::Render`
with stderr tail, `.tmp` dir removed.

- [ ] **Step 1: Write the failing tests**:

```rust
#[test]
fn thumbs_argv_is_pinned() {
    let argv = thumbs_argv(Path::new("/a/clip.mp4"), Path::new("/b/thumbs.tmp"));
    assert_eq!(argv, vec![
        "ffmpeg", "-nostdin", "-loglevel", "error", "-y",
        "-i", "/a/clip.mp4",
        "-vf", "fps=1/2,scale=320:-2", "-q:v", "5",
        "/b/thumbs.tmp/%05d.jpg",
    ]);
}

#[tokio::test]
#[ignore = "spawns real ffmpeg; owner checkpoint (KATTO_TEST_CLIP)"]
async fn generate_thumbs_real_clip() { /* env-guarded; assert count > 0, thumbs/00001.jpg exists, no thumbs.tmp */ }
```

- [ ] **Step 2: Run to verify failure**, **Step 3: Implement**, **Step 4: Run** —
  `cargo test -p katto-engine thumbs` → PASS.
- [ ] **Step 5: Commit**

```bash
git add crates/katto-engine/src/thumbs.rs crates/katto-engine/src/lib.rs
git commit -m "feat(engine): regenerable 2s-cadence timeline thumbnails"
```

---

### Task 8: Version allocation + export orchestration (`timelines.rs`)

**Files:**
- Create: `crates/katto-engine/src/timelines.rs`
- Modify: `crates/katto-engine/src/lib.rs`, `error.rs`

**Interfaces:**
- Consumes: Tasks 2/4/6 (`effective_cut_spans`, `retime_kept_words`+`group_captions`+
  `emit_srt`/`emit_vtt`, `emit_fcpxml`); `bundle::{write_atomic}`.
- Produces:

```rust
/// Next free version in `timelines/`: max N over `<slug>-v<N>.<any ext>` + 1 (1 when none).
pub fn next_version(timelines_dir: &Path, slug: &str) -> u32;

#[derive(Debug, Clone, PartialEq)]
pub struct ExportPaths {
    pub fcpxml: PathBuf, pub srt: PathBuf, pub vtt: PathBuf, pub version: u32,
}

/// Emit + validate FCPXML and captions, then write all three atomically at
/// `<timelines_dir>/<slug>-v<N>.{fcpxml,srt,vtt}`. Nothing is written when any
/// emitter/validation step fails; existing versions are never touched.
pub fn export_timeline(bundle: &Bundle, timelines_dir: &Path, slug: &str) -> Result<ExportPaths>;
```

`next_version` parsing: filename must start `<slug>-v`, then digits to the extension dot
(any extension counts — an existing `-v3.srt` blocks fcpxml v3 too). Non-matching files
ignored. Captions: transcript required (`Error::Bundle("no transcript yet")` otherwise);
retime over the same `effective_cut_spans` used by the FCPXML build. Relocation guard:
`bundle::open` already yields `SourceMissing` — export never runs against a missing
source (the caller opens the bundle checked).

- [ ] **Step 1: Write the failing tests**:

```rust
#[test]
fn next_version_scans_any_extension_and_ignores_noise() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["demo-v1.fcpxml", "demo-v3.srt", "demo-v2.fcpxml", "other-v9.fcpxml", "demo-vX.fcpxml", "notes.txt"] {
        std::fs::write(dir.path().join(name), b"x").unwrap();
    }
    assert_eq!(next_version(dir.path(), "demo"), 4);
    assert_eq!(next_version(dir.path(), "fresh"), 1);
}

#[test]
fn export_writes_all_three_versioned_and_never_overwrites() {
    let (dir, bundle) = disk_bundle_25fps(&[(1.0, 2.0)]);   // helper: real files incl. source + transcript
    let timelines = dir.path().join("timelines");
    std::fs::create_dir(&timelines).unwrap();
    let one = export_timeline(&bundle, &timelines, "demo").unwrap();
    assert_eq!(one.version, 1);
    let first = std::fs::read(&one.fcpxml).unwrap();
    let two = export_timeline(&bundle, &timelines, "demo").unwrap();
    assert_eq!(two.version, 2);
    assert_eq!(std::fs::read(&one.fcpxml).unwrap(), first);   // v1 untouched
    assert!(one.srt.exists() && one.vtt.exists());
    assert!(!timelines.join("demo-v1.fcpxml.tmp").exists());
}

#[test]
fn export_aborts_before_any_write_on_emitter_failure() {
    let (dir, bundle) = disk_bundle_25fps_all_removed();    // cuts cover everything
    let timelines = dir.path().join("timelines");
    std::fs::create_dir(&timelines).unwrap();
    assert!(export_timeline(&bundle, &timelines, "demo").is_err());
    assert_eq!(std::fs::read_dir(&timelines).unwrap().count(), 0);
}
```

  (`disk_bundle_*` helpers write a real `.kruproj` in a tempdir: fake source file,
  `project.json` at 25fps, a minimal 3-word transcript whose token boundaries the cut
  tuples land on, cuts.json — reuse Task-2 `test_support` values, now persisted with
  `bundle::write_json_atomic`.)

- [ ] **Step 2: Run to verify failure**, **Step 3: Implement**, **Step 4: Run** —
  `cargo test -p katto-engine timelines` → PASS, then `cargo test -p katto-engine`
  (whole crate).
- [ ] **Step 5: Commit**

```bash
git add crates/katto-engine/src/timelines.rs crates/katto-engine/src/lib.rs crates/katto-engine/src/error.rs
git commit -m "feat(engine): versioned timeline export orchestration, never overwriting"
```

---

### Task 9: CLI — `katto render` / `katto export`

**Files:**
- Modify: `crates/katto-cli/src/cli.rs`, `src/main.rs`, `src/output.rs`
  [P4-CONFIRM the whole CLI crate shape — Task 14 of the phase-4 plan created
  `Cli`/`Command`/`output.rs` render fns and reserved these variant slots]
- Modify: `crates/katto-cli/tests/cli.rs`

**Interfaces:**
- Consumes: engine `render::render_mp4`, `timelines::{export_timeline, next_version}`,
  `bundle::open`.
- Produces (completing the PRD CLI surface):

```rust
// appended to the existing #[derive(clap::Subcommand)] enum Command:
    /// Render the kept-only MP4 for a bundle.
    Render { bundle: PathBuf, #[arg(short, long)] out: Option<PathBuf> },
    /// Export FCPXML + SRT/VTT into the project's timelines/ directory.
    Export { bundle: PathBuf },
```

Path resolution (shared helper in `cli.rs`, unit-tested):
`project_context(bundle_root) -> (timelines_dir, slug)` — when the bundle sits at
`<project>/audio/<x>.kruproj`, timelines_dir = `<project>/timelines` (created if
missing) and slug = the project folder name; for a loose bundle, timelines_dir =
`<bundle parent>/timelines`, slug = bundle basename without `.kruproj`. `render` default
out: `<project>/exports/<slug>-render-v<N>.mp4` via `next_version` over `exports/`
(documented decision: exports reuse the never-overwrite versioning; PRD only mandates it
for timelines, consistency is free). Progress: human mode prints a single
carriage-return percent line; `--json` prints the outcome object only.

- [ ] **Step 1: Write the failing tests** — `project_context` unit tests (bundle under
  `audio/` vs loose) in `cli.rs` `mod tests`; insta snapshots for
  `render_export(&ExportPaths, json)` and `render_render(&Path, json)` in `output.rs`;
  an assert_cmd case: `katto export /nonexistent.kruproj` → exit 1, stderr non-empty.

```rust
#[test]
fn project_context_detects_audio_parent() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("proj").join("audio").join("c.kruproj");
    std::fs::create_dir_all(&root).unwrap();
    let (timelines, slug) = project_context(&root);
    assert_eq!(timelines, dir.path().join("proj").join("timelines"));
    assert_eq!(slug, "proj");
}

#[test]
fn project_context_loose_bundle_falls_back_to_sibling() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("c.kruproj");
    std::fs::create_dir_all(&root).unwrap();
    let (timelines, slug) = project_context(&root);
    assert_eq!(timelines, dir.path().join("timelines"));
    assert_eq!(slug, "c");
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p katto-cli`.
- [ ] **Step 3: Implement** command bodies (open bundle checked → `SourceMissing`
  surfaces as exit-1 error text naming the expected path; export → engine
  `export_timeline` → print paths; render → engine `render_mp4`).
- [ ] **Step 4: Run** — `cargo test -p katto-cli` → PASS (accept snapshots).
- [ ] **Step 5: Commit**

```bash
git add crates/katto-cli/
git commit -m "feat(cli): render and export subcommands complete the pipeline surface"
```

---

### Task 10: App commands (`commands/editor.rs`) + asset-protocol scope

**Files:**
- Modify: `src-tauri/tauri.conf.json` [P4-CONFIRM: phase 4 may already have enabled the
  asset protocol for `<video>` playback — if so, only widen the scope if needed]
- Create: `src-tauri/src/commands/editor.rs`
- Modify: `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs` (register)

**Invoke the `add-tauri-command` skill before writing the commands.**

**Interfaces:**
- Consumes: engine Tasks 2/7/8 + `bundle::{open, open_unchecked, save_edits}`
  [P4-CONFIRM]; jobs (`state.jobs.spawn`, `ctx.progress` — verified in this tree);
  settings repo (`db::settings::{get,set}`, key `default_nle` — exists);
  `db::events::record` + `broadcast::events_appended`;
  `tauri_plugin_opener::OpenerExt` (`reveal_item_in_dir`) — for `open_in_fcp` use
  `std::process::Command::new("open").args(["-a", "Final Cut Pro", path])` (PRD-literal;
  non-zero exit → fall back to reveal + note in the returned outcome).
- Produces:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum NleTarget { FinalCut, Resolve, Premiere }
// stored in settings as its snake_case string; only FinalCut has an open action this
// phase — Resolve/Premiere selections export identically and fall back to reveal.

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ExportPreview { pub slug: String, pub version: u32, pub default_nle: Option<NleTarget> }

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ExportResult {
    pub fcpxml_path: String, pub srt_path: String, pub vtt_path: String,
    pub version: u32, pub opened_in_nle: bool, pub revealed: bool,
}

#[tauri::command] #[specta::specta]
pub async fn save_edits(state: State<'_, AppState>, bundle_path: String, edits: katto_engine::schema::Edits) -> Result<()>;
// spawn_blocking -> engine bundle::save_edits (atomic). THE debounced auto-save target.

#[tauri::command] #[specta::specta]
pub async fn preview_export(state: State<'_, AppState>, bundle_path: String) -> Result<ExportPreview>;

#[tauri::command] #[specta::specta]
pub async fn export_timeline(
    state: State<'_, AppState>, bundle_path: String, nle_target: NleTarget, open_after: bool,
) -> Result<ExportResult>;
// checked open (SourceMissing surfaces typed) -> engine export_timeline -> persist
// nle_target to settings "default_nle" (sticky last-used, D20) -> events row
// "timeline_exported" {slug, version, fcpxml} + broadcast -> optional open/reveal.

#[tauri::command] #[specta::specta]
pub async fn render_mp4(
    state: State<'_, AppState>, bundle_path: String, out: Option<String>,
    on_progress: tauri::ipc::Channel<crate::jobs::JobProgress>,
) -> Result<jobs_repo::Job>;
// jobs.spawn("render_mp4", "Render — <name>", payload, work): work calls engine
// render_mp4 with on_progress = ctx.progress + channel send; default out via the same
// exports/ versioning rule as the CLI (shared engine-side? no — small local helper).

#[tauri::command] #[specta::specta]
pub async fn generate_thumbs(
    state: State<'_, AppState>, bundle_path: String,
    on_progress: tauri::ipc::Channel<crate::jobs::JobProgress>,
) -> Result<jobs_repo::Job>;
// jobs.spawn("generate_thumbs", …): engine generate_thumbs (no granular progress —
// one 0.5 tick after spawn, 1.0 on completion); returns count in the done payload.

#[tauri::command] #[specta::specta]
pub async fn relocate_source(state: State<'_, AppState>, bundle_path: String, new_path: String) -> Result<()>;
// Probe the new file (reuse engine ffprobe argv + parse_probe_timing), then the PURE
// check `relocation_matches(manifest, probed_duration, new_path) -> Result<()>`
// (unit-tested below): file_name must equal the manifest's, duration within one frame.
// On match, rewrite manifest.source_video_absolute_path atomically. Mismatch -> typed
// Error::Relocate naming which check failed.

#[tauri::command] #[specta::specta]
pub async fn open_in_fcp(app: tauri::AppHandle, path: String) -> Result<bool>;
// `open -a "Final Cut Pro" <path>`; Ok(false) + reveal fallback when FCP missing.

#[tauri::command] #[specta::specta]
pub async fn reveal_timeline(app: tauri::AppHandle, path: String) -> Result<()>;
```

`tauri.conf.json` addition (unless phase 4 already added it):

```json
"security": {
  "csp": null,
  "assetProtocol": { "enable": true, "scope": ["$HOME/**", "/Volumes/**"] }
}
```

- [ ] **Step 1: Write the failing tests** (in `editor.rs` `mod tests` — pure helpers
  only, command shells stay thin):

```rust
#[test]
fn relocation_requires_same_filename_and_duration() {
    let m = manifest_25fps("clip.mp4");   // helper: ProjectManifest literal
    assert!(relocation_matches(&m, m.duration, Path::new("/elsewhere/clip.mp4")).is_ok());
    assert!(relocation_matches(&m, m.duration, Path::new("/elsewhere/other.mp4")).is_err());
    let off = m.duration.checked_add(Rational::new(2, 1)).unwrap();
    assert!(relocation_matches(&m, off, Path::new("/elsewhere/clip.mp4")).is_err());
}

#[test]
fn nle_target_round_trips_snake_case() {
    assert_eq!(serde_json::to_string(&NleTarget::FinalCut).unwrap(), "\"final_cut\"");
    assert_eq!(serde_json::from_str::<NleTarget>("\"final_cut\"").unwrap(), NleTarget::FinalCut);
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p katto editor`.
- [ ] **Step 3: Implement** commands + conf change; register all seven in
  `collect_commands![]`. Duration tolerance for relocation: within one frame
  (`fps.den/fps.num`).
- [ ] **Step 4: Regenerate bindings + gate** — `just check` (runs `export_bindings`;
  bindings diff shows the commands + `NleTarget`/`ExportPreview`/`ExportResult`).
- [ ] **Step 5: Commit**

```bash
git add src-tauri/ src/lib/ipc/bindings.gen.ts Cargo.lock
git commit -m "feat(app): editor command domain - save, export, render, thumbs, relocate"
```

---

### Task 11: Frontend IPC wrappers + seconds↔Rational wire model

**Files:**
- Create: `src/lib/ipc/editor.ts`
- Create: `src/features/editor/model/wire.ts`, `wire.test.ts`
- Modify: `package.json` (add deps)

**Interfaces:**
- Consumes: generated bindings after Task 10 (`commands.saveEdits`,
  `commands.previewExport`, `commands.exportTimeline`, `commands.renderMp4`,
  `commands.generateThumbs`, `commands.relocateSource`, `commands.openInFcp`,
  `commands.revealTimeline`; types `Edits`, `Rational`, `NleTarget`, `ExportPreview`,
  `ExportResult`, `Job`, `JobProgress`) — confirm exact generated names in
  `bindings.gen.ts` (tauri-specta camelCases).
- Produces:

```ts
// src/lib/ipc/editor.ts — thin typed wrappers, the only invoke site for this domain
export function saveEdits(bundlePath: string, edits: Edits): Promise<null>;
export function previewExport(bundlePath: string): Promise<ExportPreview>;
export function exportTimeline(bundlePath: string, nleTarget: NleTarget, openAfter: boolean): Promise<ExportResult>;
export function renderMp4(bundlePath: string, out: string | null, onProgress: (p: JobProgress) => void): Promise<Job>;
export function generateThumbs(bundlePath: string, onProgress: (p: JobProgress) => void): Promise<Job>;
export function relocateSource(bundlePath: string, newPath: string): Promise<null>;
export function openInFcp(path: string): Promise<boolean>;
export function revealTimeline(path: string): Promise<null>;

// src/features/editor/model/wire.ts — pure, no React
export type Range = { start: number; end: number };            // seconds, UI domain
export type ManualCutSec = Range & { note: string | null };
export type BoundaryAdjustmentSec = { cutIndex: number; edge: "start" | "end"; newTime: number };
export type EditorDocument = {
  toggledOff: number[];
  appliedDiscretionary: number[];
  manualCuts: ManualCutSec[];
  boundaryAdjustments: BoundaryAdjustmentSec[];
};
export type EditorHistory = { past: EditorDocument[]; future: EditorDocument[] };

/** Frame-exact seconds -> Rational in the video frame timebase (den = fps.num). */
export function secondsToRational(sec: number, fps: Rational): Rational;
export function rationalToSeconds(r: Rational): number;
/** Document <-> Edits wire (including history <-> EditHistory), depth-capped at 100. */
export function toWireEdits(doc: EditorDocument, history: EditorHistory, fps: Rational): Edits;
export function fromWireEdits(edits: Edits | null, fps: Rational): { document: EditorDocument; history: EditorHistory };
```

`secondsToRational`: `num = Math.round(sec * fps.num)` wait — ticks/s = fps.num means
`num = Math.round(sec * fps.num)`, `den = fps.num`… that is only frame-exact when sec is
already frame-aligned times `fps.den`. Correct construction (documented in the file):
frame index `k = Math.round(sec * fps.num / fps.den)`; result `{ num: k * fps.den,
den: fps.num }` — exactly the engine's frame grid.

- [ ] **Step 1: Add dependencies**

```bash
bun add zundo wavesurfer.js
```

  (zustand `^5.0.14` already present; expect zundo `^2.3.0`, wavesurfer `^7.12`.)

- [ ] **Step 2: Write the failing tests** (`wire.test.ts`):

```ts
const NTSC: Rational = { num: 30000, den: 1001 };

it("secondsToRational lands on the frame grid", () => {
  expect(secondsToRational(0.5, NTSC)).toEqual({ num: 15 * 1001, den: 30000 }); // frame 15
  expect(secondsToRational(0, NTSC)).toEqual({ num: 0, den: 30000 });
});

it("wire round-trip preserves the document and caps history at 100", () => {
  const doc: EditorDocument = {
    toggledOff: [1], appliedDiscretionary: [0],
    manualCuts: [{ start: 1.5, end: 2.5, note: null }],
    boundaryAdjustments: [{ cutIndex: 0, edge: "end", newTime: 4.0 }],
  };
  const history = { past: Array.from({ length: 130 }, () => doc), future: [doc] };
  const wire = toWireEdits(doc, history, { num: 25, den: 1 });
  expect(wire.history?.past).toHaveLength(100);
  const back = fromWireEdits(wire, { num: 25, den: 1 });
  expect(back.document).toEqual(doc);
  expect(back.history.future).toHaveLength(1);
});

it("fromWireEdits of null yields an empty document", () => {
  const { document, history } = fromWireEdits(null, { num: 25, den: 1 });
  expect(document.toggledOff).toEqual([]);
  expect(history.past).toEqual([]);
});
```

- [ ] **Step 3: Run to verify failure** — `bunx vitest run src/features/editor/model/wire.test.ts`.
- [ ] **Step 4: Implement** wire.ts + editor.ts wrappers (wrappers are declarative
  passthroughs; no tests beyond tsc, per the IPC-layer convention).
- [ ] **Step 5: Run** — vitest PASS + `bunx tsc --noEmit`.
- [ ] **Step 6: Commit**

```bash
git add src/lib/ipc/editor.ts src/features/editor/model/wire.ts \
        src/features/editor/model/wire.test.ts package.json bun.lock
git commit -m "feat(editor): typed editor IPC and seconds-rational wire model"
```

---

### Task 12: Pure edit ops, snap math, kept-range math (`model/`)

**Files:**
- Create: `src/features/editor/model/cut-ops.ts`, `cut-ops.test.ts`
- Create: `src/features/editor/model/snap.ts`, `snap.test.ts`
- Create: `src/features/editor/model/kept-ranges.ts`, `kept-ranges.test.ts`

**Interfaces:**
- Consumes: `EditorDocument`/`Range` (Task 11), `TokenSpan` from phase-4
  `model/tokens.ts` [P4-CONFIRM: `{index, text, kind, start, end, speakerId}`], wire
  `Cuts` type from bindings.
- Produces:

```ts
// cut-ops.ts — every op returns a NEW document (zundo diffs object identity)
export function toggleCut(doc: EditorDocument, cutIndex: number): EditorDocument;      // add/remove in toggledOff
export function applyDiscretionary(doc: EditorDocument, index: number): EditorDocument; // add/remove in appliedDiscretionary
export function addManualCut(doc: EditorDocument, range: Range, tokens: TokenSpan[]): EditorDocument; // outward-snapped
export function removeManualCut(doc: EditorDocument, index: number): EditorDocument;
export function adjustBaseBoundary(doc: EditorDocument, cutIndex: number, edge: "start" | "end", newTime: number): EditorDocument; // upsert by (cutIndex, edge)
export function adjustManualBoundary(doc: EditorDocument, index: number, edge: "start" | "end", newTime: number): EditorDocument;
/** Dragging an applied-discretionary edge converts it: un-apply + manual cut at the
 *  adjusted range (the wire format has no discretionary boundary mechanism). */
export function convertDiscretionaryDrag(doc: EditorDocument, cuts: Cuts, index: number, edge: "start" | "end", newTime: number): EditorDocument;

// snap.ts
/** Nearest word-boundary time (any token's start/end) to t; null when none within maxDelta. */
export function nearestTokenBoundary(t: number, tokens: TokenSpan[], maxDelta: number): number | null;
/** Snap for region-edge drags: token boundary unless free=true (Option held), then frame grid. */
export function snapEdge(t: number, tokens: TokenSpan[], fps: Rational, free: boolean): number;
/** Manual-cut ranges snap OUTWARD into adjacent spacing tokens (PRD): start moves left
 *  to the start of a covering/preceding spacing token, end moves right likewise. */
export function snapOutward(range: Range, tokens: TokenSpan[]): Range;

// kept-ranges.ts — mirrors engine merge + keep_windows semantics in seconds
export function effectiveCutRanges(cuts: Cuts, doc: EditorDocument): Array<Range & { key: string }>;
// base minus toggledOff, + applied discretionary, + manual, boundary adjustments
// applied to base cuts, inverted spans dropped, sorted; key = "base-0"|"disc-1"|"manual-2"
export function coalesceRanges(ranges: Range[]): Range[];
export function keptRanges(cutRanges: Range[], duration: number): Range[];
export function keptDuration(cutRanges: Range[], duration: number): number;
/** Kept-only playback: if t is inside a cut (with 5ms boundary guard), the seek target
 *  just past it, else null. Mirrors the audio-editor timeupdate pattern. */
export function seekPastCut(t: number, coalesced: Range[]): number | null;
```

- [ ] **Step 1: Write the failing tests** (representative — write the full set):

```ts
// cut-ops.test.ts
const doc0: EditorDocument = { toggledOff: [], appliedDiscretionary: [], manualCuts: [], boundaryAdjustments: [] };

it("toggleCut toggles membership and returns new objects", () => {
  const a = toggleCut(doc0, 2);
  expect(a.toggledOff).toEqual([2]);
  expect(toggleCut(a, 2).toggledOff).toEqual([]);
  expect(a).not.toBe(doc0);
});

it("adjustBaseBoundary upserts by cut and edge", () => {
  const a = adjustBaseBoundary(doc0, 0, "end", 4.0);
  const b = adjustBaseBoundary(a, 0, "end", 4.2);
  expect(b.boundaryAdjustments).toEqual([{ cutIndex: 0, edge: "end", newTime: 4.2 }]);
});

it("convertDiscretionaryDrag un-applies and adds an adjusted manual cut", () => {
  const withDisc = applyDiscretionary(doc0, 0);
  const out = convertDiscretionaryDrag(withDisc, fixtureCuts, 0, "end", 11.8); // disc[0] = 9.0..11.2
  expect(out.appliedDiscretionary).toEqual([]);
  expect(out.manualCuts).toEqual([{ start: 9.0, end: 11.8, note: null }]);
});

// snap.test.ts (tokens: word 0.12-0.34, spacing 0.34-0.47, word 0.47-0.90, spacing 0.90-1.0, word 1.0-1.4)
it("nearestTokenBoundary finds the closest edge within maxDelta", () => {
  expect(nearestTokenBoundary(0.45, tokens, 0.1)).toBe(0.47);
  expect(nearestTokenBoundary(5.0, tokens, 0.1)).toBeNull();
});

it("snapOutward expands into adjacent spacing tokens", () => {
  // selecting the middle word (0.47..0.90) expands to the spacing envelope 0.34..1.0
  expect(snapOutward({ start: 0.47, end: 0.9 }, tokens)).toEqual({ start: 0.34, end: 1.0 });
});

it("snapEdge honors free drag by snapping to the frame grid only", () => {
  expect(snapEdge(0.451, tokens, { num: 25, den: 1 }, false)).toBe(0.47);
  expect(snapEdge(0.451, tokens, { num: 25, den: 1 }, true)).toBeCloseTo(0.44, 5); // frame 11.275 -> 11 -> 0.44
});

// kept-ranges.test.ts
it("effectiveCutRanges mirrors the engine merge semantics", () => {
  const doc = { ...doc0, toggledOff: [0], manualCuts: [{ start: 7, end: 8, note: null }] };
  const out = effectiveCutRanges(fixtureCuts, doc); // base [ (1,2)=idx0, (4,5)=idx1 ]
  expect(out.map((r) => r.key)).toEqual(["base-1", "manual-0"]);
});

it("keptRanges complements and seekPastCut jumps with the 5ms guard", () => {
  const cuts = [{ start: 1, end: 2 }];
  expect(keptRanges(cuts, 5)).toEqual([{ start: 0, end: 1 }, { start: 2, end: 5 }]);
  expect(seekPastCut(1.5, cuts)).toBeCloseTo(2.001, 3);
  expect(seekPastCut(1.997, cuts)).toBeNull();   // inside the 5ms boundary guard
  expect(seekPastCut(0.5, cuts)).toBeNull();
});
```

- [ ] **Step 2: Run to verify failure** — `bunx vitest run src/features/editor/model` → fails.
- [ ] **Step 3: Implement** (no React imports; `noUncheckedIndexedAccess`-clean; keep
  each file under ~120 lines).
- [ ] **Step 4: Run** — PASS.
- [ ] **Step 5: Commit**

```bash
git add src/features/editor/model/
git commit -m "feat(editor): pure edit ops, token snap, and kept-range math"
```

---

### Task 13: Editor document store (zundo) + debounced auto-save

**Files:**
- Create: `src/features/editor/store/editor-store.ts`, `editor-store.test.ts`
- Create: `src/features/editor/store/autosave.ts`, `autosave.test.ts`

**Interfaces:**
- Consumes: Tasks 11/12; `zundo` `temporal`; zustand `createStore` (vanilla) +
  `useStore`.
- Produces (the `.claude/rules/frontend.md` editor-store contract made concrete):

```ts
// editor-store.ts
export type EditorState = EditorDocument & {
  // actions (not in history — partialize strips them):
  toggleCut(cutIndex: number): void;
  applyDiscretionary(index: number): void;
  addManualCut(range: Range): void;
  removeManualCut(index: number): void;
  dragBoundary(target: DragTarget, edge: "start" | "end", newTime: number): void; // interim, paused
  beginDrag(): void;
  commitDrag(): void;
  cancelDrag(): void;
  replaceDocument(doc: EditorDocument): void;   // relocation/undo-external use
};
export type DragTarget = { kind: "base"; cutIndex: number } | { kind: "manual"; index: number } | { kind: "disc"; index: number };

export type EditorStore = ReturnType<typeof createEditorStore>;
/** One store per opened bundle; history seeded from edits.json at creation
 *  (documented zundo init-time pastStates/futureStates). */
export function createEditorStore(init: {
  document: EditorDocument;
  history: EditorHistory;
  cuts: Cuts;                    // for convertDiscretionaryDrag
  tokens: TokenSpan[];           // for outward snap inside addManualCut
}): StoreApi<EditorState> & { temporal: TemporalStore };

export function documentOf(s: EditorState): EditorDocument;  // the partialize projection
```

Drag coalescing (the zundo caveat from the verified-contracts section, exactly):
`beginDrag()` → snapshot `preDrag = documentOf(get())`, `temporal.pause()`. Interim
`dragBoundary(...)` calls set state while paused (no history). `commitDrag()` → capture
`final = documentOf(get())`; **while still paused** `set(preDrag)`; `temporal.resume()`;
`set(final)` — exactly one history entry whose past state is preDrag. `cancelDrag()` →
`set(preDrag)`, `resume()`. `limit: 100`; `partialize` keeps only the four document
arrays; `equality` = deep-equal on the partialized doc (skips no-op sets).

```ts
// autosave.ts — pure-ish controller, timer injected for tests
export type SaveFn = (edits: Edits) => Promise<unknown>;
export type AutosaveState = "idle" | "pending" | "saving" | "paused-error";
export function createAutosave(opts: {
  store: EditorStore; fps: Rational; save: SaveFn; debounceMs?: number;  // default 200
  onPaused(message: string): void;   // banner hook (disk full etc.)
}): { flushNow(): Promise<void>; dispose(): void; state(): AutosaveState };
```

Behavior: subscribes to the store AND to `store.temporal` (undo/redo must also persist —
history travels in the same `toWireEdits(doc, {past, future}, fps)` payload); debounce
200 ms; save failure → immediate single retry → on second failure `paused-error` +
`onPaused(message)`; any later successful `flushNow()` (user acknowledges banner)
resumes. `flushNow` used by the close guard (Task 18).

- [ ] **Step 1: Write the failing tests**:

```ts
// editor-store.test.ts
it("each discrete edit is one undo step, Cmd+Z semantics", () => {
  const store = createEditorStore(init());
  store.getState().toggleCut(0);
  store.getState().applyDiscretionary(0);
  expect(store.temporal.getState().pastStates).toHaveLength(2);
  store.temporal.getState().undo();
  expect(documentOf(store.getState()).appliedDiscretionary).toEqual([]);
  expect(documentOf(store.getState()).toggledOff).toEqual([0]);
});

it("a drag of many ticks is exactly one history entry with pre-drag past state", () => {
  const store = createEditorStore(init());
  store.getState().beginDrag();
  for (const t of [4.1, 4.2, 4.3, 4.4]) store.getState().dragBoundary({ kind: "base", cutIndex: 0 }, "end", t);
  store.getState().commitDrag();
  expect(store.temporal.getState().pastStates).toHaveLength(1);
  store.temporal.getState().undo();
  expect(documentOf(store.getState()).boundaryAdjustments).toEqual([]);
});

it("history seeds from edits.json and survives recreation (restart simulation)", () => {
  const a = createEditorStore(init());
  a.getState().toggleCut(0);
  const wire = toWireEdits(documentOf(a.getState()), historyOf(a), { num: 25, den: 1 });
  const b = createEditorStore({ ...init(), ...fromWireEdits(wire, { num: 25, den: 1 }) });
  expect(b.temporal.getState().pastStates).toHaveLength(1);
  b.temporal.getState().undo();
  expect(documentOf(b.getState()).toggledOff).toEqual([]);
});

// autosave.test.ts (vi.useFakeTimers)
it("coalesces edits into one save 200ms after the last change", async () => {
  const save = vi.fn().mockResolvedValue(null);
  const { dispose } = createAutosave({ store, fps, save, onPaused: vi.fn() });
  store.getState().toggleCut(0);
  store.getState().toggleCut(1);
  await vi.advanceTimersByTimeAsync(199);
  expect(save).not.toHaveBeenCalled();
  await vi.advanceTimersByTimeAsync(1);
  expect(save).toHaveBeenCalledTimes(1);
  expect(save.mock.calls[0][0].history).toBeDefined();
  dispose();
});

it("save failure retries once then pauses with a banner", async () => {
  const save = vi.fn().mockRejectedValue(new Error("disk full"));
  const onPaused = vi.fn();
  createAutosave({ store, fps, save, onPaused });
  store.getState().toggleCut(0);
  await vi.advanceTimersByTimeAsync(200);
  await vi.runAllTimersAsync();
  expect(save).toHaveBeenCalledTimes(2);
  expect(onPaused).toHaveBeenCalledOnce();
});
```

- [ ] **Step 2: Run to verify failure** — `bunx vitest run src/features/editor/store`.
- [ ] **Step 3: Implement** (store ~120 lines, autosave ~80).
- [ ] **Step 4: Run** — PASS.
- [ ] **Step 5: Commit**

```bash
git add src/features/editor/store/
git commit -m "feat(editor): zundo document store with coalesced drags and 200ms autosave"
```

---

### Task 14: Transcript pane — editing interactions

**Files:**
- Modify: `src/features/editor/transcript-pane.tsx`, `transcript-pane.test.tsx`
  [P4-CONFIRM current props: `{transcript, cuts, onSeek}` and the `<del>`/`<mark>`
  overlay markup from phase-4 Task 18]
- Create: `src/features/editor/model/selection.ts`, `selection.test.ts`

**Interfaces:**
- Consumes: Task 12 ops via new callbacks; phase-4 `buildTokenSpans`/`classifyTokens`.
- Produces:

```ts
// transcript-pane.tsx — props grow (read-only mode stays: callbacks optional)
export function TranscriptPane(props: {
  transcript: Transcript;
  cuts: Cuts | null;
  document: EditorDocument | null;        // effective overlay = classifyTokens over effectiveCutRanges
  activeTime: number;                     // karaoke highlight (current token)
  onSeek(seconds: number): void;
  onToggleCut?(cutIndex: number): void;         // click struck word
  onApplyDiscretionary?(index: number): void;   // click amber span
  onManualCut?(range: Range): void;             // drag-select + X/Delete (snapped outward by the store)
  selectedKey?: string | null;                  // timeline<->transcript sync highlight
}): JSX.Element;

// model/selection.ts — pure
/** Map a DOM selection's [anchorTokenIndex, focusTokenIndex] to a time range spanning
 *  those tokens (order-normalized); null when collapsed. */
export function tokenRangeToTime(tokens: TokenSpan[], a: number, b: number): Range | null;
```

Behavior (PRD row "Transcript editing", exact): click a struck (cut) word →
`onToggleCut(entryIndex)` — the cut toggles off and the word renders plain. Decision
(PRD says "toggle that cut off"): a struck-word click only ever disables; a plain word
click is seek-only. Re-enabling a toggled-off cut happens via undo (Cmd+Z) — the
one-directional click keeps a plain transcript click from silently re-cutting words. Click amber (discretionary) span →
`onApplyDiscretionary(index)` — it becomes a real cut (struck). Applied-discretionary
tokens render struck like cuts (effective overlay computed from `effectiveCutRanges`,
not raw `cuts.json`). Drag-select tokens + `X`/`Delete` keydown → `onManualCut(range)`
via `tokenRangeToTime` over `window.getSelection()` anchor/focus token elements (each
token span carries `data-token-index`). Click yellow flag → seek only (never a cut —
unchanged). Active token (`activeTime` inside its span) gets a `--surface-2` background
(karaoke sync, from design grounding); `selectedKey` region's tokens get the same
treatment plus `scrollIntoView({block: "nearest"})` on change.

- [ ] **Step 1: Write the failing tests** (extend the phase-4 test file; RTL, behavior
  only):

```ts
it("clicking a struck word toggles its cut off", () => {
  const onToggleCut = vi.fn();
  render(<TranscriptPane {...base} document={emptyDoc} onToggleCut={onToggleCut} />);
  fireEvent.click(screen.getByText("So"));            // inside cuts[0] per fixture
  expect(onToggleCut).toHaveBeenCalledWith(0);
});

it("clicking an amber span applies the discretionary cut and it renders struck", () => {
  const onApply = vi.fn();
  const { rerender } = render(<TranscriptPane {...base} document={emptyDoc} onApplyDiscretionary={onApply} />);
  fireEvent.click(screen.getByText("today"));         // discretionary[0]
  expect(onApply).toHaveBeenCalledWith(0);
  rerender(<TranscriptPane {...base} document={{ ...emptyDoc, appliedDiscretionary: [0] }} onApplyDiscretionary={onApply} />);
  expect(screen.getByText("today").closest("del")).not.toBeNull();
});

it("selection + X creates a manual cut over the selected tokens", () => {
  const onManualCut = vi.fn();
  render(<TranscriptPane {...base} document={emptyDoc} onManualCut={onManualCut} />);
  selectTokens("today", "Next");                      // test helper: set window selection over the spans
  fireEvent.keyDown(document, { key: "x" });
  expect(onManualCut).toHaveBeenCalledWith({ start: 0.47, end: 3.2 });
});

// selection.test.ts
it("tokenRangeToTime normalizes order and spans token extents", () => {
  expect(tokenRangeToTime(tokens, 3, 1)).toEqual({ start: tokens[1].start, end: tokens[3].end });
  expect(tokenRangeToTime(tokens, 2, 2)).toEqual({ start: tokens[2].start, end: tokens[2].end });
});
```

- [ ] **Step 2: Run to verify failure**, **Step 3: Implement**, **Step 4: Run** —
  `bunx vitest run src/features/editor` → PASS.
- [ ] **Step 5: Commit**

```bash
git add src/features/editor/transcript-pane.tsx src/features/editor/transcript-pane.test.tsx \
        src/features/editor/model/selection.ts src/features/editor/model/selection.test.ts
git commit -m "feat(editor): transcript editing - toggle, apply, drag-select manual cuts"
```

---

### Task 15: Kept-only playback + transport

**Files:**
- Create: `src/features/editor/transport.ts`, `transport.test.ts`
- Create: `src/features/editor/use-transport.ts`
- Modify: `src/features/editor/video-pane.tsx` [P4-CONFIRM `VideoPaneHandle`]
- Modify: `src/features/editor/editor-view.tsx`

**Interfaces:**
- Consumes: Task 12 `seekPastCut`/`coalesceRanges`/`effectiveCutRanges`; store;
  `temporal.undo/redo`.
- Produces:

```ts
// transport.ts — pure key map (PRD row "Kept-only playback", exact bindings)
export type TransportAction =
  | { kind: "toggle-play" } | { kind: "stop" }
  | { kind: "play-forward" }                    // L; stacking bumps rate to 2
  | { kind: "shuttle-back" }                    // J; reverse-ish scrub
  | { kind: "step-frames"; frames: number }     // ←/→ = ±1, Shift = ±10
  | { kind: "undo" } | { kind: "redo" }
  | { kind: "manual-cut" }                      // X / Delete with a live selection
  | { kind: "toggle-original" };                // O (documented; PRD names a toggle, binds it here)
export function keyToAction(e: Pick<KeyboardEvent, "key" | "code" | "shiftKey" | "metaKey">): TransportAction | null;

// use-transport.ts — wires a <video> element + store
export function useTransport(opts: {
  videoRef: RefObject<VideoPaneHandle | null>;
  getCutRanges(): Range[];        // coalesced effective ranges (empty when showOriginal)
  fps: Rational;
}): {
  showOriginal: boolean; toggleOriginal(): void;
  currentTime: number; playing: boolean; rate: number;
  onTimeUpdate(t: number): void;  // kept-only seek-past (5ms guard) via seekPastCut
  dispatch(a: TransportAction): void;
};
```

JKL semantics under HTML5 constraints (PRD-acknowledged): `K` = pause. `L` = play; a
second `L` while playing sets `playbackRate = 2` (stacking cap). `J` = reverse-ish
shuttle: pause, then an interval stepping `currentTime` back 0.25 s every 250 ms (≈1×
reverse); any other transport action clears the interval. Space = toggle play/pause.
Frame step = pause + seek `±frames × fps.den/fps.num`. `Cmd+Z`/`Cmd+Shift+Z` → temporal
undo/redo. Video-pane grows `getCurrentTime/setRate/getRate` on its handle
[P4-CONFIRM existing handle then extend]. Transport row UI (design grounding): one
32 px row under the video — ghost play/pause + `J K L` no-chrome keys legend omitted
(keyboard-first; no decorative legend), mono `tabular-nums` `m:ss.s / m:ss.s`
current/total readout (kept-time when kept-only, source-time when showing original), a
"show original" ghost toggle whose state reads as pressed (`aria-pressed`), and the
zoom slider (Task 16) right-aligned.

- [ ] **Step 1: Write the failing tests** (`transport.test.ts` — pure map only; the
  hook is exercised through Task 16's integration and the owner checkpoint):

```ts
it.each([
  [{ key: " ", code: "Space", shiftKey: false, metaKey: false }, { kind: "toggle-play" }],
  [{ key: "k", code: "KeyK", shiftKey: false, metaKey: false }, { kind: "stop" }],
  [{ key: "l", code: "KeyL", shiftKey: false, metaKey: false }, { kind: "play-forward" }],
  [{ key: "j", code: "KeyJ", shiftKey: false, metaKey: false }, { kind: "shuttle-back" }],
  [{ key: "ArrowRight", code: "ArrowRight", shiftKey: false, metaKey: false }, { kind: "step-frames", frames: 1 }],
  [{ key: "ArrowLeft", code: "ArrowLeft", shiftKey: true, metaKey: false }, { kind: "step-frames", frames: -10 }],
  [{ key: "z", code: "KeyZ", shiftKey: false, metaKey: true }, { kind: "undo" }],
  [{ key: "z", code: "KeyZ", shiftKey: true, metaKey: true }, { kind: "redo" }],
  [{ key: "x", code: "KeyX", shiftKey: false, metaKey: false }, { kind: "manual-cut" }],
  [{ key: "o", code: "KeyO", shiftKey: false, metaKey: false }, { kind: "toggle-original" }],
])("maps %o", (e, expected) => expect(keyToAction(e)).toEqual(expected));

it("ignores keys while typing in an input", () => {
  // keyToAction is pure; the hook guards on event.target — assert the guard helper
  expect(isEditableTarget(document.createElement("input"))).toBe(true);
  expect(isEditableTarget(document.createElement("div"))).toBe(false);
});
```

- [ ] **Step 2: Run to verify failure**, **Step 3: Implement** transport.ts +
  use-transport.ts + video-pane handle extension + editor-view wiring (keydown listener
  on the editor surface, `isEditableTarget` guard), **Step 4: Run** —
  `bunx vitest run src/features/editor` + `just check`.
- [ ] **Step 5: Commit**

```bash
git add src/features/editor/transport.ts src/features/editor/transport.test.ts \
        src/features/editor/use-transport.ts src/features/editor/video-pane.tsx \
        src/features/editor/editor-view.tsx
git commit -m "feat(editor): kept-only playback with JKL transport and frame stepping"
```

---

### Task 16: Timeline pane — geometry model + canvas track + 3-pane layout

**Files:**
- Create: `src/features/editor/model/timeline-geometry.ts`, `timeline-geometry.test.ts`
- Create: `src/features/editor/timeline-pane.tsx`
- Modify: `src/features/editor/editor-view.tsx` (3-pane grid per the Layout decision)

**Interfaces:**
- Consumes: Tasks 12/13/15; `generateThumbs` IPC + `convertFileSrc` for
  `<bundle>/thumbs/%05d.jpg`.
- Produces:

```ts
// model/timeline-geometry.ts — pure; ALL interaction math lives here, the component
// only draws and forwards pointer events
export type Viewport = { pxPerSec: number; scrollSec: number; widthPx: number };
export function timeToPx(t: number, vp: Viewport): number;
export function pxToTime(x: number, vp: Viewport): number;
export function clampViewport(vp: Viewport, duration: number): Viewport;
export function zoomAround(vp: Viewport, anchorPx: number, factor: number, duration: number): Viewport;
/** Ruler ticks: major every 1/2/5/10/30/60s picked so labels are >=70px apart. */
export function rulerTicks(vp: Viewport, duration: number): Array<{ t: number; major: boolean }>;
export type RegionRect = { key: string; startPx: number; endPx: number; kind: "cut" | "discretionary" };
export function regionRects(ranges: Array<Range & { key: string }>, discretionaryUnapplied: Range[], vp: Viewport): RegionRect[];
export type HitTarget =
  | { kind: "edge"; key: string; edge: "start" | "end" }
  | { kind: "region"; key: string }
  | { kind: "empty"; t: number };
/** Edge grab zone ±4px, else region body, else empty. */
export function hitTest(xPx: number, rects: RegionRect[], vp: Viewport): HitTarget;
/** Thumb tiles for the filmstrip: which %05d.jpg covers each 2s slot in view. */
export function thumbSlots(vp: Viewport, duration: number): Array<{ index: number; xPx: number; wPx: number }>;
```

`timeline-pane.tsx` (component, deliberately thin): a `<canvas>` sized by
ResizeObserver with HiDPI handling (`canvas.width = cssWidth * devicePixelRatio`,
`ctx.scale(dpr, dpr)`, re-run on `dpr` and resize changes); draw order per frame:
filmstrip thumbnails (Image cache keyed by index, `convertFileSrc` URLs, drawn dimmed
`globalAlpha 0.85`) → cut regions (fill `--bg` at 75% + 45°-hatch pattern via an
offscreen 8px tile — *absence*, not color; discretionary-unapplied rendered as a dotted
`--warn` outline only) → selected region 1px `--ember` outline → ruler ticks + mono
labels (12px `--mono`, `--fg-faint`) → playhead (1px `--fg` line + 3×8px top handle).
Colors read from `getComputedStyle` custom properties — tokens, never literals.
Interactions (pointer events → geometry model → store): drag an edge = `beginDrag()` +
`dragBoundary(target, edge, snapEdge(t, tokens, fps, e.altKey))` per move +
`commitDrag()` on up; drag-select on empty = marquee → on up ≥ 0.05 s → `X` key or
release-with-selection creates manual cut (store `addManualCut` handles outward snap);
click a region = select (`selectedKey` shared with transcript); click empty = seek.
Wheel = horizontal scroll; `Shift+wheel` or the transport slider = `zoomAround`. A
"Generate thumbnails" ghost action overlays the strip when `thumbs/` is missing;
auto-triggered once per bundle open when absent (jobs-framework call, non-blocking).

- [ ] **Step 1: Write the failing tests** (geometry only — canvas drawing is not
  unit-tested per testing rules):

```ts
const vp = { pxPerSec: 50, scrollSec: 10, widthPx: 500 };

it("time<->px round-trips through the viewport", () => {
  expect(timeToPx(12, vp)).toBe(100);
  expect(pxToTime(100, vp)).toBe(12);
});

it("zoomAround keeps the anchor time stationary", () => {
  const z = zoomAround(vp, 250, 2, 600);
  expect(pxToTime(250, z)).toBeCloseTo(pxToTime(250, vp), 6);
  expect(z.pxPerSec).toBe(100);
});

it("rulerTicks picks a step keeping labels >=70px apart", () => {
  const ticks = rulerTicks(vp, 600);          // 50px/s -> 2s step (100px)
  const majors = ticks.filter((t) => t.major).map((t) => t.t);
  expect(majors[1]! - majors[0]!).toBe(2);
});

it("hitTest prefers edges within 4px, then bodies, then empty", () => {
  const rects = [{ key: "base-0", startPx: 100, endPx: 160, kind: "cut" as const }];
  expect(hitTest(103, rects, vp)).toEqual({ kind: "edge", key: "base-0", edge: "start" });
  expect(hitTest(130, rects, vp)).toEqual({ kind: "region", key: "base-0" });
  expect(hitTest(300, rects, vp)).toEqual({ kind: "empty", t: pxToTime(300, vp) });
});

it("thumbSlots maps 2s cadence to 1-based %05d indices", () => {
  const slots = thumbSlots({ pxPerSec: 50, scrollSec: 0, widthPx: 300 }, 600);
  expect(slots[0]).toEqual({ index: 1, xPx: 0, wPx: 100 });
  expect(slots[1]!.index).toBe(2);
});
```

- [ ] **Step 2: Run to verify failure**, **Step 3: Implement** geometry → PASS, then the
  component + the editor-view 3-pane grid (transcript scroll column | pinned video;
  bottom strip: ruler+canvas+waveform slot+transport). `bunx tsc --noEmit` clean.
- [ ] **Step 4: Run** — `bunx vitest run src/features/editor` + `just check`.
- [ ] **Step 5: Commit**

```bash
git add src/features/editor/model/timeline-geometry.ts src/features/editor/model/timeline-geometry.test.ts \
        src/features/editor/timeline-pane.tsx src/features/editor/editor-view.tsx
git commit -m "feat(editor): canvas timeline with thumbnails, region drag, and token snap"
```

---

### Task 17: Waveform strip (wavesurfer v7)

**Files:**
- Create: `src/features/editor/waveform.tsx`

**Interfaces:**
- Consumes: verified wavesurfer 7.12 contract (see Verified external contracts — the
  gotcha list is normative); store + Task 12 ranges; `convertFileSrc` for
  `cached_audio.wav`.
- Produces:

```tsx
export function Waveform(props: {
  audioUrl: string;                       // convertFileSrc(<bundle>/cached_audio.wav)
  ranges: Array<Range & { key: string }>; // effective cut ranges
  currentTime: number;                    // drives ws.setTime (display only)
  viewport: Viewport;                     // shared zoom/scroll with the canvas track
  selectedKey: string | null;
  onSeek(t: number): void;                // interaction event -> video seek
  onDragBegin(): void;                    // store.beginDrag
  onDrag(key: string, edge: "start" | "end", t: number): void;
  onDragEnd(): void;                      // store.commitDrag
  onSelect(key: string | null): void;
}): JSX.Element;
```

Implementation notes (each traces to a verified gotcha): wavesurfer is DISPLAY + region
editing only — `ws.play()` is never called; the `<video>` owns playback and
`currentTime` drives `ws.setTime`. Options: `{container, url: audioUrl, height: 72,
waveColor: token(--fg-faint), progressColor: token(--fg-muted), cursorColor:
token(--ember), barWidth: 2, barGap: 1, barRadius: 1, minPxPerSec: viewport.pxPerSec,
normalize: true, interact: true, dragToSeek: false, autoCenter: false}`. Regions sync
effect gated on `isReady && ws.getDuration() > 0`; own-id pre-tagging before
`addRegion({id: key, start, end, color: cutFill, drag: false, resize: true})`;
`region-update` with `region.updatingSide` set → first tick calls `onDragBegin()`;
`region-updated` → `onDrag(key, side, value)` + `onDragEnd()` (side from the event's
second arg, fallback `updatingSide`); programmatic `setOptions` never trips the loop
(no `updatingSide`). `region-clicked` → `onSelect(key)`; `interaction` → `onSeek(t)`.
Viewport sync: `ws.zoom(viewport.pxPerSec)` + `ws.setScroll(viewport.scrollSec *
pxPerSec)` in an effect; wavesurfer `scroll`/`zoom` events are NOT fed back (the canvas
owns the viewport — one-way sync avoids the feedback loop). Region fill uses the same
dimmed treatment as the canvas (rgba of `--bg` at 0.55); selected region gets a
`setOptions({color})` bump. No shadow-root chrome injected this phase (regions carry no
content labels — the canvas track already labels regions; less chrome, design
restraint).

- [ ] **Step 1: No new unit tests** — wavesurfer is an integration surface (jsdom has
  no AudioContext); behavior is covered by the store/geometry tests it delegates to and
  the owner checkpoint. `bunx tsc --noEmit` is the gate here, per testing rules
  ("component tests assert user-visible behavior" — none is assertable in jsdom for
  canvas/audio).
- [ ] **Step 2: Implement** + mount in `editor-view.tsx` between canvas track and
  transport.
- [ ] **Step 3: Run** — `bunx tsc --noEmit` + `bunx vitest run src/features/editor`
  (existing suites stay green) + `just check`.
- [ ] **Step 4: Commit**

```bash
git add src/features/editor/waveform.tsx src/features/editor/editor-view.tsx
git commit -m "feat(editor): wavesurfer waveform strip mirroring cut regions"
```

---

### Task 18: Export dialog, NLE seeding, relocation, close guard

**Files:**
- Create: `src/features/editor/export-dialog.tsx`, `export-dialog.test.tsx`
- Create: `src/features/editor/relocate-dialog.tsx`
- Modify: `src/features/editor/editor-view.tsx`

**Interfaces:**
- Consumes: Task 11 IPC (`previewExport`, `exportTimeline`, `renderMp4`, `openInFcp`,
  `revealTimeline`, `relocateSource`); Task 13 `flushNow`; existing dialog/button
  primitives in `src/components/ui/` (reuse — check before writing new ones);
  `@tauri-apps/api/window` `getCurrentWindow().onCloseRequested` and
  `@tauri-apps/plugin-dialog`-style open picker [CONFIRM-AT-IMPL: how phase 1-3 pick
  files — reuse the same mechanism (`plugin-opener` has no picker; if no dialog plugin
  exists, add `@tauri-apps/plugin-dialog` + capability, or use a hidden
  `<input type="file">`; match the ingest surface's existing pattern)].
- Produces:

```tsx
export function ExportDialog(props: {
  bundlePath: string;
  onClose(): void;
  onExported(result: ExportResult): void;
}): JSX.Element;
```

Dialog behavior (PRD rows "NLE target" + "Versioning" + user story, exact): on open →
`flushNow()` then `previewExport` → shows the version line in mono
(`nvme-deep-dive-v3.fcpxml` — the real next filename) plus "v1 and v2 untouched"-style
count line when version > 1. Formats: "Timeline (FCPXML 1.11)" checked+disabled (always
written), "Captions (SRT + VTT)" checked+disabled (written by the same command — shown
so the output list is honest), "Also render MP4" unchecked checkbox → after export
spawns the `renderMp4` job (progress lives in the jobs surface, not the dialog). NLE
target: radio list from `NleTarget` (Final Cut Pro / DaVinci Resolve / Premiere Pro);
preselected from `preview.default_nle`; **no stored default → no preselection and the
Export button stays disabled until a pick is made** (first-export forced choice, D20);
the choice always persists as the new sticky default (command side). Primary button
"Export" → `exportTimeline(bundle, target, openAfter)`; `openAfter` comes from a
secondary action: after success the dialog swaps to a done state — mono file list +
"Open in Final Cut" primary (only when target is FinalCut; else "Reveal in Finder") —
per D20/PRD the open action targets the selected NLE. Errors render inline in plain
language (engine invariant text verbatim), dialog stays open. `SourceMissing` error →
closes into the relocate dialog.

`relocate-dialog.tsx`: shown when `open_bundle` or export fails with the
`SourceMissing` payload [P4-CONFIRM error wire shape `{kind, message}` — the message
carries expected path + filename + duration; if phase 4 exposes structured fields, use
them]. Shows expected filename + duration; "Locate file…" → picker filtered to the
filename; `relocateSource` → success reloads the bundle; mismatch error text names the
failed check (filename vs duration) verbatim from the typed error.

Close guard (D21 activation — live edit state now exists): while autosave state is
`pending`/`saving`, `onCloseRequested` handler `preventDefault()`s, awaits `flushNow()`,
then `getCurrentWindow().destroy()` [CONFIRM-AT-IMPL against `src-tauri/src/window.rs`
teardown — the handler must not fight the phase-2 close-to-tray flow; if the Rust side
owns close, expose the flush as a `beforeunload`-time best-effort `saveEdits` call as
well].

- [ ] **Step 1: Write the failing tests** (`export-dialog.test.tsx`, mockIPC):

```ts
it("forces an NLE pick on first export and previews the exact filename", async () => {
  mockPreview({ slug: "nvme-deep-dive", version: 3, default_nle: null });
  render(<ExportDialog bundlePath="/b" onClose={noop} onExported={noop} />);
  expect(await screen.findByText("nvme-deep-dive-v3.fcpxml")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Export" })).toBeDisabled();
  fireEvent.click(screen.getByRole("radio", { name: /Final Cut/ }));
  expect(screen.getByRole("button", { name: "Export" })).toBeEnabled();
});

it("preselects the sticky default and exports through it", async () => {
  mockPreview({ slug: "demo", version: 1, default_nle: "final_cut" });
  const exportSpy = mockExportTimeline();
  render(<ExportDialog bundlePath="/b" onClose={noop} onExported={noop} />);
  expect(await screen.findByRole("radio", { name: /Final Cut/ })).toBeChecked();
  fireEvent.click(screen.getByRole("button", { name: "Export" }));
  await waitFor(() => expect(exportSpy).toHaveBeenCalledWith("/b", "final_cut", false));
});

it("offers Open in Final Cut after a successful export", async () => {
  mockPreview({ slug: "demo", version: 1, default_nle: "final_cut" });
  mockExportTimeline({ version: 1, fcpxml_path: "/t/demo-v1.fcpxml", srt_path: "/t/demo-v1.srt", vtt_path: "/t/demo-v1.vtt", opened_in_nle: false, revealed: false });
  render(<ExportDialog bundlePath="/b" onClose={noop} onExported={noop} />);
  fireEvent.click(await screen.findByRole("button", { name: "Export" }));
  expect(await screen.findByRole("button", { name: "Open in Final Cut" })).toBeInTheDocument();
});
```

- [ ] **Step 2: Run to verify failure**, **Step 3: Implement** dialogs + editor-view
  wiring ("Export…" secondary button in the transport row's right cluster, before the
  zoom slider) + close guard, **Step 4: Run** — `bunx vitest run src/features/editor` +
  `just check`.
- [ ] **Step 5: Commit**

```bash
git add src/features/editor/export-dialog.tsx src/features/editor/export-dialog.test.tsx \
        src/features/editor/relocate-dialog.tsx src/features/editor/editor-view.tsx
git commit -m "feat(editor): export dialog with sticky NLE target and source relocation"
```

---

### Task 19: Phase close-out — reviewers, gate, owner checklist

**Files:**
- Modify: `docs/overnight-run.md` (Phase 5 section — untracked file, NEVER staged)
- Modify: `prd/index.md` (phase-5 status)

- [ ] **Step 1: Run the reviewers** — `rust-reviewer` on the crates/src-tauri diff,
  `frontend-reviewer` on the src/ diff; fix confirmed findings (the overnight
  adversarial review agent also runs — coordinate via the team lead).
- [ ] **Step 2: Full gate** — `just check` from the workspace root; paste the output
  tail in the task report. Must be green.
- [ ] **Step 3: Append to `docs/overnight-run.md`:**

```markdown
## Phase 5 — Cut Editor & Export

_Status: implemented, pending review_

### Owner visual/manual checks (tick after testing)

- [ ] EDITOR WALKTHROUGH: open a planned bundle — three panes render (transcript left,
      video right, timeline+waveform strip below); click a struck word -> cut toggles
      off everywhere (transcript, timeline, waveform); click an amber span -> becomes a
      real cut; select a sentence, press X -> manual cut snapped outward into spacing;
      click a yellow flag -> seek only.
- [ ] KEPT-ONLY PLAYBACK: playback skips cuts audibly/visibly; "show original" restores
      everything; Space/J/K/L behave (L twice = 2x, J = reverse scrub), arrow frame
      steps land on frames, Shift+arrows jump 10.
- [ ] TIMELINE: thumbnails populate (auto-generated on first open); drag a cut edge —
      snaps to word boundaries, Option = free drag; drag-select empty area + X = manual
      cut; region click highlights + scrolls the transcript span.
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
- [ ] DTD GATE (once): `cargo test -p katto-engine fcpxml -- --ignored` passes on this
      machine (xmllint against FCP's shipped FCPXMLv1_11.dtd).
- [ ] MP4 RENDER: "Also render MP4" spawns a jobs-row render; output plays in
      QuickTime, cuts absent, audio in sync at cut points; job failure (e.g. yank the
      source) surfaces stderr tail, bundle intact.
- [ ] CAPTIONS SANITY: open the .srt/.vtt — timestamps are kept-time (first caption at
      ~0), lines break at sentences / ~42 chars, no text from removed spans.
- [ ] RELOCATION: move the source file, reopen the bundle -> relocation dialog names
      the missing file; picking the moved file (same name) heals the bundle; picking a
      wrong file is refused with the failed check named.
- [ ] CLI PARITY: `katto export <bundle>` and `katto render <bundle>` produce the same
      artifacts headless; `katto render` on an everything-cut bundle fails loud.
- [ ] Events log shows `timeline_exported {slug, version}` rows; render/thumbs appear
      as jobs with tray progress.
- [ ] Ignored hardware tests when convenient:
      `KATTO_TEST_CLIP=/path/clip.mp4 cargo test -p katto-engine -- --ignored`
      (render_real_clip_end_to_end, generate_thumbs_real_clip).

Design deviations from the plan (for review): <fill during implementation — list every
divergence, or "none">.
```

- [ ] **Step 4: Update `prd/index.md`** — phase-5 status cell → "implemented, pending
  verification" (same wording convention as phases 3/4).
- [ ] **Step 5: Commit** (prd/index.md only — overnight-run.md and this plan stay
  untracked/unstaged)

```bash
git add prd/index.md
git commit -m "docs(prd): mark phase 5 cut editor and export implemented pending verification"
```

---

## Self-Review (performed while writing — verified against `prd/phase-5.md`)

Coverage map, PRD scope row → task: Transcript editing (toggle / amber apply /
drag-select X with outward snap / flag seek-only) → 12/14 · Kept-only playback
(timeupdate seek-past, show original, Space/JKL/arrows) → 12/15 · Timeline pane (canvas,
2s thumbs regenerable, dimmed/hatched + discretionary dotted, frame-accurate edge drag
with token snap + Option free, drag-select + X, region↔transcript sync) → 7/16 ·
Waveform (wavesurfer v7 over cached_audio.wav via asset protocol, regions mirroring
cuts, all four normative gotchas encoded) → 17 · Undo/redo (one edit per step, drags
coalesced via the pause/restore/resume pattern, depth 100, Cmd+Z/Cmd+Shift+Z, history in
edits.json surviving restart) → 3/13/15 · Auto-save (200 ms debounce, single save_edits
bridge call, dirty close guard, disk-full retry→pause+banner) → 13/18 · Relocation
(filename+duration match, manifest-only rewrite) → 10/18 · FCPXML 1.11 (quick-xml
Writer + typed builder, one sequence at source rate, DF/NDF, percent-encoded file URL,
rational times in the format timebase, decimal = validation failure) → 5/6 · Rescue
track (every removed segment as a connected lane="-1" enabled="0" clip, own snapshot
fixture) → 6 · Versioning (max+1, never overwrite, validate-before-write, events row) →
8/10 · NLE target (dialog surfaces target, sticky default_nle, first-export forced
pick, persisted on change) → 10/18 · Open in FCP (`open -a`, missing-FCP reveal
fallback) → 10/18 · MP4 render (cut-video verbatim math, 6-decimal graph, script file,
500+ cuts fixture, always re-encode, loud whole-duration error) → 1/2 · SRT/VTT
(kept-only retiming, 42-char/sentence grouping, outputs beside the timeline export) →
4/8 · CLI render/export → 9 · Error-handling table → 6 (abort-before-write, invariant
named), 2/10 (ffmpeg stderr tail via jobs), 13 (disk-full retry→banner), 10/18
(SourceMissing→dialog, FCP-missing→reveal+note) · Testing section → insta goldens
(basic/rescue/DF/500+/unicode) in 1/4/5/6, keep-window parity tables in 1, retiming
units in 4, model/store/undo-coalescing/partialize in 12/13, xmllint DTD gate
(`#[ignore]`, run once in Task 6) + manual FCP/Resolve import in 19 · Data-model deltas:
none (settings key `default_nle` already exists; edits.json gains an optional field —
bundle files, no DB migration) · Out of scope respected: no transcript text editing, no
multi-clip, no B-roll/caption layers, no Premiere dialect, no Resolve scripting, no
dock re-route.

Type-consistency spot-checks: `EditorDocument`/`EditorHistory` defined once (Task 11),
consumed by 12/13/14; `Range {start, end}` seconds everywhere in the UI; engine spans
`(Rational, Rational)` everywhere; `effective_cut_spans` shared by render (2), fcpxml
(6), captions-at-export (8); `Viewport` owned by the canvas (16), one-way synced into
wavesurfer (17); `NleTarget` snake_case on the wire in 10/11/18.

