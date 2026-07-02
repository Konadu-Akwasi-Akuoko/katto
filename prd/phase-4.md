# Phase 4 — Cut Pipeline

## Goal

A footage clip goes in; a validated, reviewable AI cut plan comes out — visible in the app
(interactive transcript, cuts overlaid read-only) and runnable headless via `katto cut`.
This phase turns `katto-engine` from two stub files into the real pipeline (old milestones
M1–M3, plus the read-only half of M5).

## Why this order

The engine is the app's reason to exist, and everything in it is testable without hardware or
UI polish. Read-only review ships before editing (Phase 5) so the pipeline is exercised
end-to-end while the editor surface is still simple.

## External inputs (local-only files; see prd/README.md §Source availability)

| Input | Where it lives (owner's machine) | What this PRD needs from it |
|---|---|---|
| Cut-decider system prompt | `agents/cut-decider.md` (gitignored) | Its body becomes the planner system prompt **verbatim** (committed into the engine as `crates/katto-engine/prompts/cut-decider.md` at implementation time). Defines the two-pass method + cut policy (fillers, stutters, false starts, retakes, self-corrections, silences >1.0 s leaving ~0.3 s, audio events, low-confidence `logprob < -7.0` → flag never cut). |
| Validation reference | `skills/clean-audio/scripts/schemas.ts` + `scripts/__tests__/fixtures/` (gitignored) | The Zod validators and fixtures (`cuts.valid.json`, `cuts.overlap.json`, `cuts.out-of-bounds.json`, `transcript.valid.json`) — port to Rust as the validator test vectors (copied into `crates/katto-engine/tests/fixtures/`). |

The full contracts are stated below, so the phase is implementable from this document alone;
porting the originals is the preferred (higher-fidelity) route when working on the owner's
machine.

## Scope with acceptance criteria

| Feature | Acceptance criteria |
|---|---|
| `Rational` complete | Add/sub/mul, `from_seconds(f64, timebase: u32)` (round to nearest frame), `to_secs_f64`, `rescale(den)`, frame snap, ordering; property-tested round-trips; no silent precision loss (den preserved through arithmetic) |
| Schema types real | Fields `pub`; `Cut.reason: CutReason` and `Discretionary.reason: DiscretionaryReason` (serde snake_case); `Edits.boundary_adjustments: Vec<BoundaryAdjustment {cut_index, edge: Start\|End, new_time: Rational}>`; `Transcript` types for the Scribe v2 shape (`{audio_duration_secs?, language_code, language_probability, text, words[]}`, `words` tagged by `type`: `word {text, start, end, speaker_id, logprob?}` \| `spacing` \| `audio_event`) |
| cuts.json validation | Full contract below; violation → typed `ValidationError` naming the invariant + offending entry |
| Float↔Rational boundary | Model/transcript decimal seconds convert to `Rational` in the **video frame timebase** exactly once, at ingest into engine state; UI receives `f64` projections; nothing mid-pipeline re-derives floats |
| `import` | `ffprobe` (`r_frame_rate` — not `avg_frame_rate` — duration, streams) → `project.json`; `ffmpeg -vn -ar 16000 -ac 1` → `cached_audio.wav`; creates `.kruproj/` bundle (layout below) inside the owning project folder under `audio/`; standalone/loose bundle open supported |
| `transcribe` | POST `cached_audio.wav` to ElevenLabs Scribe v2 (`/v1/speech-to-text`, multipart; word timestamps, diarize, audio-event tagging on); key from keychain; full response stored as `transcript.json`; network/auth/quota failures typed + retryable; **no partial writes** (atomic tmp→rename) |
| `CutPlanner` trait | `async fn plan(&self, transcript: &Transcript) -> Result<Cuts, PlanError>`; single-shot prompt+transcript → JSON (no agent loop, D14) |
| `SubprocessClaudePlanner` | Spawns `claude --print --output-format json` with the cut-decider body as system prompt (exact flag spelling is impl detail), transcript JSON on stdin; default when `claude` detected |
| `HttpAnthropicPlanner` | Anthropic Messages API, model default `claude-sonnet-4-6` (settings-overridable), `max_tokens 8192`, key from keychain |
| Validate-retry loop | Parse+validate planner output; on invalid JSON or invariant violation, retry **once** with the error appended ("the JSON you returned was invalid: <error>; return only valid JSON matching the schema"); second failure surfaces raw output as a debugging aid |
| cuts↔edits merge | `effective_cuts(cuts, edits) -> Vec<Cut>`: base cuts minus toggled-off, plus applied discretionary, plus manual cuts, with boundary adjustments applied; pure, deterministic, exhaustively tested |
| Bundle round-trip | open → mutate edits → save → reopen → identical state |
| CLI | `katto import <video> [--project <dir>]`, `katto transcribe <bundle>`, `katto plan <bundle>`, `katto cut <video>` (import+transcribe+plan), `katto auth status` (claude detection + key presence); clap; human-readable + `--json` output; exit codes: 0 ok, 1 pipeline error, 2 usage |
| Pipeline UI | Project detail footage card → "Plan rough cut" → step indicator (Extracting audio ≈s → Transcribing ≈30 s/5 min → Detecting cuts) as one job streaming over `Channel<PipelineEvent>`; transcript rendered and interactive (click word → seek) as soon as transcription lands, before planning finishes; cuts appear incrementally as planner JSON parses |
| Read-only review | Token-aligned spans: `cuts[]` gray strikethrough, `discretionary[]` amber dotted underline, `flags[]` yellow highlight; click flag → seek only; `<video>` via `convertFileSrc` (media never over `invoke`) |

## cuts.json contract (normative)

Shape (decimal seconds at the model boundary):

```json
{
  "source_duration_secs": 312.48,
  "cuts":          [{"start": 4.21, "end": 4.68, "reason": "filler", "excerpt": "um"}],
  "discretionary": [{"start": 9.0, "end": 11.2, "reason": "other", "excerpt": "…",
                     "note": "tangent about X", "confidence": "medium"}],
  "flags":         [{"start": 40.1, "end": 40.4, "reason": "low_confidence",
                     "excerpt": "NVMe", "logprob": -7.8}],
  "total_cut_secs": 41.937
}
```

`reason` enums — cuts: `filler|stutter|false_start|self_correction|long_silence|audio_event`;
discretionary adds `other`; flags: always `low_confidence`. `confidence`: `low|medium|high`.

Invariants (each independently validated; violation names the entry):

1. Every cut/discretionary/flag: `0 ≤ start < end ≤ source_duration_secs`.
2. `cuts[]` sorted by `start`; no overlap (`cuts[i].end ≤ cuts[i+1].start`).
3. `discretionary[]` never overlaps any `cuts[]` entry.
4. `flags[]` never shares a span with `cuts[]` (flagged words are **never** cut).
5. Every flag has `logprob`; every discretionary has non-empty `note` + valid `confidence`.
6. `total_cut_secs` = Σ cut durations within **0.001 s** (cuts only, not discretionary).
7. **Token alignment:** every cut boundary equals a token boundary in `transcript.words`
   (a word's `start` or `end`; boundaries may fall in `spacing` tokens, never inside a
   `word` token — no clipped syllables).

## `.kruproj` bundle layout

```
<project>/audio/<basename>.kruproj/
  project.json      # source path (absolute), frame_rate (Rational), duration, schema_version
  transcript.json   # full Scribe v2 response, cached
  cached_audio.wav  # mono 16 kHz
  cuts.json         # planner output (validated)
  edits.json        # user overrides + persistent undo log (Phase 5 writes it)
  thumbs/           # timeline thumbnails (Phase 5, regenerable)
```

Source video referenced by absolute path; missing on open → relocatable (Phase 5 dialog;
engine returns `SourceMissing {expected_path, filename, duration}` from open).

## Backend (Rust) — engine layout

```
crates/katto-engine/src/
  lib.rs           # pub surface re-exports (#![warn(missing_docs)])
  rational.rs      # complete Rational
  schema.rs + schema/   # transcript.rs, cuts.rs, edits.rs, manifest.rs
  validate.rs      # cuts.json invariants (pure)
  bundle.rs        # .kruproj open/save (atomic), relocation support
  import.rs        # ffprobe/ffmpeg orchestration (parsers pure, spawns thin)
  transcribe.rs    # ElevenLabs client (reqwest)
  planner.rs + planner/ # trait, subprocess.rs, http.rs, retry.rs
  merge.rs         # cuts↔edits effective view
  prompts/cut-decider.md  # committed system prompt (external input)
```

Crates: `reqwest` (rustls), `serde`/`serde_json`, `thiserror`, `insta` + `rstest` + `proptest`
(dev). Engine takes the API key as an argument — keychain stays in the app/CLI layers
(engine never depends on tauri; CLI uses the keychain via a small shared helper or env var).

App side: `commands/pipeline.rs` (`plan_rough_cut(project_slug, footage_path, Channel<PipelineEvent>)`
→ jobs-framework job driving import→transcribe→plan), `PipelineEvent = Stage {name, progress}
| TranscriptReady {bundle_path} | CutsPartial {cuts_so_far} | Done {bundle_path} | Failed {error}`.

## Frontend (React)

`src/features/pipeline/` (step indicator, progress); `src/features/editor/` starts here:
`model/tokens.ts` (token span building, char↔time maps — pure), `model/overlay.ts`
(cut/discretionary/flag span classification — pure), `transcript-pane.tsx` (word spans,
click-to-seek), `video-pane.tsx` (HTML5 `<video>` + `convertFileSrc`). Read-only: no edit
handlers yet. Bundle loads in **one** command returning the whole parsed bundle (IPC rule).

## Wiring / IPC

| Command | Notes |
|---|---|
| `plan_rough_cut(slug, footage_path, on_event: Channel<PipelineEvent>) -> job_id` | the pipeline job |
| `open_bundle(path) -> BundleData` | whole bundle in one payload (`project`, `transcript`, `cuts`, `edits`) |
| `list_bundles(slug) -> Vec<BundleSummary>` | project detail listing |
| CLI equivalents | same engine calls, no IPC |

## Data-model deltas

None (bundles are files; pipeline runs are `jobs` rows).

## Error handling

Per original-spec §8, all typed, surfaced via job failure + events (never blocking dialogs):
ElevenLabs failure (retryable, no partial transcript) · planner invalid JSON/invariants
(retry-once then surface raw output) · subprocess `claude` non-zero (stderr surfaced; offer
API-key mode) · ffmpeg/ffprobe failure (stderr surfaced; bundle state intact) · source missing
on open (typed, relocation deferred to Phase 5) · disk full on save (retry once, then surface).

## Testing

- `Rational`: unit + proptest (`from_seconds`∘`to_secs` round-trip within frame tolerance;
  arithmetic den preservation).
- Validators: table-driven over ported fixtures (`cuts.valid.json`, `cuts.overlap.json`,
  `cuts.out-of-bounds.json`, plus new `cuts.misaligned-token.json`, `cuts.flag-overlap.json`,
  `cuts.bad-total.json`).
- Merge: exhaustive unit tests (toggle off, discretionary applied, manual overlapping-with-
  toggled, boundary adjustments).
- Pipeline integration: mocked ElevenLabs (canned `transcript.valid.json`) + mocked planner
  (canned cuts) → bundle end-to-end without network; retry loop with an invalid-then-valid
  planner stub.
- Bundle round-trip; ffprobe parser over captured JSON fixtures (incl. 29.97 DF rates).
- Real-network tests behind `--features expensive-tests` (`#[ignore]` fallback), run manually.
- CLI: snapshot tests (insta) for output + exit codes against fixtures.
- Frontend: token/overlay model unit tests; transcript pane click-to-seek with `mockIPC`.
- Manual checkpoint: real ZV-E10 II 4K clip through the full pipeline in-app.

## Out of scope

All editing interactions, undo, auto-save (Phase 5); exports (5); timeline/waveform panes (5);
dock re-route of planning (6); caption/MP4 outputs (5).

## Exit criteria

Drag a real 4K clip onto a project → transcript + validated AI cut plan visible in-app, words
click-seek the video; `katto cut <video>` produces the same bundle headless; `just check` green.
