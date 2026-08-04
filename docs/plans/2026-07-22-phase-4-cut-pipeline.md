# Phase 4 — Cut Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (or
> superpowers:subagent-driven-development) to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** A footage clip goes in; a validated, reviewable AI cut plan comes out — visible
in-app (interactive transcript, cuts overlaid read-only) and runnable headless via
`katto cut` (PRD: `prd/phase-4.md`).

**Architecture:** All pure logic (Rational math, schemas, validators, merge, argv builders,
response parsers) lands in `crates/katto-engine`; the engine also owns the thin
ffprobe/ffmpeg/claude spawn sites and the reqwest ElevenLabs/Anthropic clients (keys passed
in as arguments — keychain stays in app/CLI). `src-tauri` adds one pipeline job command
streaming `Channel<PipelineEvent>` plus `open_bundle`/`list_bundles`. `katto-cli` grows the
clap surface. The frontend adds `features/pipeline/` (step indicator) and the read-only
start of `features/editor/` (token spans, click-to-seek, overlay classification).

**Tech Stack:** Rust 2024 workspace (serde, thiserror, tokio, reqwest 0.12 rustls,
insta/rstest/proptest/wiremock dev), Tauri 2 + tauri-specta `=2.0.0-rc.25`, clap 4.5,
React 19 + TS + Vite + Tailwind v4 + TanStack Query + Zustand, vitest + RTL, bun.

## Global Constraints

- **Gate:** `just check` (fmt-check + clippy `-D warnings` + cargo test + tsc) from the
  workspace root. Never claim a task or the phase done without it green.
- **Rational end-to-end:** engine timestamps/durations are `Rational {num, den}`. Floats
  only at UI and model/transcript boundaries. **`MediaInfo.duration_s: Option<f64>` is
  display-only — Phase 4 cut math must NEVER consume it.** Durations derive from
  `duration_ts × time_base` (or stream-level rational fields) as `Rational`.
- **Model boundary:** cuts.json / transcript.json are decimal seconds (f64) on disk and at
  the planner boundary; conversion to `Rational` in the video frame timebase happens
  exactly once (`CutPlan::from_wire`); nothing mid-pipeline re-derives floats.
- **No `unwrap()`/`expect()` outside `#[cfg(test)]`.** One thiserror enum per crate in
  `error.rs`; `anyhow` only in binary `main`.
- **Atomic artifact writes:** `<name>.tmp` → `rename`, always. No partial transcript/cuts
  writes.
- **Keys:** never logged, never written to disk outside the keychain, never returned to the
  frontend. Engine takes keys as arguments.
- **Nothing fails silently:** the pipeline runs as a `jobs` row; terminal states write
  `events` rows (the jobs framework does this automatically; add a domain event on success).
- **No numeric scoring** anywhere in AI-suggestion surfaces (confidence is the locked
  `low|medium|high` enum from the PRD — never a number rendered as a score).
- **Media never crosses `invoke`:** video plays via `convertFileSrc` asset protocol; the
  bundle loads in ONE `open_bundle` call (JSON payload only).
- **Dirty-tree discipline:** `src/components/ui/date-input.{tsx,test.tsx}`, hunks in
  `src/features/projects/detail/project-detail.{tsx,test.tsx}` and one hunk in
  `src/styles/main.css` are leftover DateInput work — **never commit them, never
  `git add -A`**. If a task must commit changes to those files, `git stash push` the
  DateInput paths first, commit, `git stash pop` (Phase 3 used this successfully).
- **Concurrent-fix caution:** `src-tauri/src/ingest/`, `src-tauri/src/volumes.rs`, and
  `src-tauri/src/commands/ingest.rs` are under a concurrent Phase-3 fix pass. Phase 4 does
  not depend on them, but any task marked **[CONFIRM-AT-IMPL]** must re-read the named file
  at implementation time instead of trusting signatures quoted here.
- **Do not start the dev app** (`bun run tauri dev`); the owner tests visually after waking.
- Conventional commits, one concern per commit, tests travel with their feature commit.
- Frontend: bun only; regenerate bindings via the `export_bindings` test (`just check` runs
  it); never hand-edit `src/lib/ipc/bindings.gen.ts`. Use the `add-tauri-command` and
  `add-feature-surface` skills where the task says so.
- PRD out-of-scope stays out: no editing interactions, no undo, no auto-save, no exports,
  no timeline/waveform panes, no dock re-route, no caption/MP4 outputs. (D21's
  beforeunload-flush requirement activates when live edit state exists — that is Phase 5's
  `edits.json` writing, not this phase; note it in the Phase 5 plan, do not build it here.)

### Verified external contracts (do not re-derive)

- **ElevenLabs Scribe v2** (verified against official docs 2026-07-22):
  `POST https://api.elevenlabs.io/v1/speech-to-text`, header `xi-api-key: <key>`,
  `multipart/form-data` fields: `file` (binary; max 5 GB, min 100 ms), `model_id` =
  `"scribe_v2"`, `timestamps_granularity` = `"word"`, `diarize` = `"true"`,
  `tag_audio_events` = `"true"`. 200 response:
  `{language_code, language_probability, text, words[], audio_duration_secs, transcription_id}`
  where each `words[]` entry is `{text, type: "word"|"spacing"|"audio_event", start, end,
  logprob?, speaker_id?, channel_index?}`. Errors: 401 unauthorized, 422 validation, 429
  rate limit. (A `"transcripts"` key means multichannel; `"request_id"` means webhook mode —
  both are hard errors for katto, same as the clean-audio reference.)
- **claude CLI** (verified locally, v2.1.217): `claude -p --output-format json` prints one
  JSON object whose fields include `result` (the reply text), `session_id`, `is_error`,
  `total_cost_usd`. `--system-prompt <text>` replaces the default system prompt;
  `--append-system-prompt <text>` appends. The prompt may come from stdin (capped 10 MB).
  `--resume <session_id>` continues a specific session (must run from the same directory).
  `--output-format stream-json --verbose --include-partial-messages` emits NDJSON with
  `stream_event` lines carrying `text_delta`s and a final `result` line.
  **Never pass `--bare`: bare mode skips OAuth/keychain auth and would break D14's
  subscription-auth design.**
- **Anthropic Messages API** (per claude-api skill): `POST https://api.anthropic.com/v1/messages`,
  headers `x-api-key`, `anthropic-version: 2023-06-01`, `content-type: application/json`;
  body `{model, max_tokens, system, messages:[{role:"user", content}]}`. Model default is
  the PRD-locked `claude-sonnet-4-6` (a valid, active model id), `max_tokens: 8192`.
  No prefill; single-shot text response at `content[0].text`; errors are
  `{type:"error", error:{type, message}}` with HTTP 401/429/400/529.
- **hyper-frames reuse map** (mirror read 2026-07-22): transcript token shape reused
  verbatim (matches `skills/clean-audio/scripts/schemas.ts` and the mirror's
  `transcribe_elevenlabs/transcript.py`). katto's cuts.json is the clean-audio /
  cut-decider shape (`source_duration_secs`/`cuts`/`discretionary`/`flags`/`total_cut_secs`),
  **not** the mirror's `tools/cut-video` `{version:1}` shape. Known divergences, all
  PRD-locked: katto uses `r_frame_rate` (mirror prefers `avg_frame_rate`); katto derives
  duration from `duration_ts × time_base` as `Rational` (mirror uses float
  `format.duration`); katto adds a retry-once planner loop (mirror validates-and-halts);
  the `.kruproj` bundle, `edits.json`, and cuts↔edits merge have **no mirror antecedent** —
  they are net-new, defined only by the PRD. The mirror's keep-window/filtergraph math
  belongs to Phase 5 (render/export) — do not port it in this phase.

### External inputs (local-only files, present on this machine — port, don't invent)

- `agents/cut-decider.md` → body (frontmatter stripped) becomes
  `crates/katto-engine/prompts/cut-decider.md` verbatim (Task 9).
- `skills/clean-audio/scripts/__tests__/__fixtures__/{cuts.valid,cuts.overlap,cuts.out-of-bounds,transcript.valid}.json`
  → `crates/katto-engine/tests/fixtures/` (Task 3/4).

---

## File Structure

```
crates/katto-engine/
  Cargo.toml                      # + reqwest, tokio features; dev: insta rstest proptest wiremock
  prompts/cut-decider.md          # NEW committed planner system prompt (external input)
  src/
    lib.rs                        # + pub mod bundle, import, merge, planner, schema/*, transcribe, validate
    error.rs                      # grows variants (Io, SourceMissing, Transcribe*, Plan, Validation, Bundle)
    rational.rs                   # completed (Task 1)
    ffprobe.rs                    # + ProbeTiming / parse_probe_timing (Task 2); MediaInfo untouched
    schema.rs                     # parent: re-exports submodules
    schema/transcript.rs          # Scribe v2 shape (Task 3)
    schema/cuts.rs                # wire cuts.json types, f64 (Task 3)
    schema/edits.rs               # edits.json types, Rational (Task 5)
    schema/manifest.rs            # project.json types (Task 6)
    validate.rs                   # cuts.json invariants, pure (Task 4)
    merge.rs                      # CutPlan + effective_cuts (Task 5)
    bundle.rs                     # .kruproj open/save, atomic writes (Task 6)
    import.rs                     # ffprobe/ffmpeg argv builders (pure) + thin spawns (Task 7)
    transcribe.rs                 # ElevenLabs client (Task 8)
    planner.rs                    # CutPlanner trait, PlanError, parse_cuts_json (Task 9)
    planner/partial.rs            # incremental cuts extractor, pure (Task 9)
    planner/subprocess.rs         # SubprocessClaudePlanner (Task 10)
    planner/http.rs               # HttpAnthropicPlanner (Task 11)
    planner/retry.rs              # validate-retry loop (Task 12)
  tests/
    fixtures/                     # ported + new fixtures, ffprobe captures, stream-json capture
    pipeline.rs                   # mocked end-to-end integration (Task 13)

crates/katto-cli/
  Cargo.toml                      # + clap, tokio, keyring; dev: assert_cmd, insta
  src/main.rs                     # thin entry
  src/cli.rs, src/keys.rs, src/output.rs   # clap surface, key resolution, render fns (Task 14)

src-tauri/src/
  keychain.rs                     # + read_key() (Task 15)
  commands.rs / commands/pipeline.rs  # plan_rough_cut, open_bundle, list_bundles (Task 15)
  commands/settings.rs            # + planner_model setting (Task 15)
  lib.rs                          # register 3 commands (Task 15)

src/
  lib/ipc/pipeline.ts             # typed wrappers (Task 16)
  stores/pipeline.ts              # pipeline job/event store (Task 16)
  features/pipeline/plan-steps.tsx     # step indicator (Task 16)
  features/editor/model/tokens.ts (+ .test.ts)    # Task 17
  features/editor/model/overlay.ts (+ .test.ts)   # Task 17
  features/editor/transcript-pane.tsx (+ .test.tsx)  # Task 18
  features/editor/video-pane.tsx                  # Task 18
  features/editor/editor-view.tsx                 # Task 18 (route target)

docs/overnight-run.md             # Phase 4 checkboxes (Task 19)
prd/index.md                      # status flip (Task 19)
```

---

### Task 1: Complete `Rational`

**Files:**
- Modify: `crates/katto-engine/src/rational.rs`
- Modify: `crates/katto-engine/Cargo.toml` (dev-deps only)

**Interfaces:**
- Produces (later tasks depend on these exact names):
  `Rational::new(num: i64, den: u32) -> Rational` ·
  `Rational::from_seconds(secs: f64, timebase: u32) -> Rational` ·
  `to_secs_f64(self) -> f64` · `rescale(self, den: u32) -> Rational` ·
  `checked_add/checked_sub(self, rhs) -> Option<Rational>` ·
  `checked_mul_int(self, k: i64) -> Option<Rational>` ·
  `snap_to_frame(self, fps: Rational) -> Rational` · `Ord`/`PartialOrd` impls.

Semantics (from the PRD acceptance row "Rational complete"):
- `from_seconds` rounds to the **nearest tick** of `timebase` (ticks per second):
  `num = (secs * timebase as f64).round() as i64`, `den = timebase`. Non-finite input maps
  to `num = 0` (documented; upstream validation prevents it reaching here).
- `checked_add`/`checked_sub`: same-den fast path keeps the den; mixed dens compute over
  the lcm (all intermediates in `i128`), returning `None` only on overflow. **No implicit
  gcd reduction of the result den** — den is the timebase and must survive arithmetic
  (PRD: "no silent precision loss (den preserved through arithmetic)").
- `checked_mul_int` scales the numerator only (den preserved).
- `rescale(den)` converts to a new timebase rounding to nearest (i128 intermediate:
  `num_new = round(num * den_new / den_old)` via `(n * d_new * 2 + d_old) / (2 * d_old)`
  style integer rounding — implement with `i128` and explicit rounding, no floats).
- `snap_to_frame(fps)`: frame index `i = round(t * fps)` computed exactly in `i128`
  (`i = round(num * fps.num / (den * fps.den))`), result is `i * fps.den / fps.num` seconds
  **rescaled back to `self.den`**.
- `Ord`: cross-multiply in `i128` (`a.num * b.den` vs `b.num * a.den`).

- [ ] **Step 1: Add dev-deps** to `crates/katto-engine/Cargo.toml`:

```toml
[dev-dependencies]
insta = { version = "1", features = ["json"] }
proptest = "1"
rstest = "0.26.1"
```

- [ ] **Step 2: Write the failing tests** in `rational.rs` under `#[cfg(test)] mod tests`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const NTSC: Rational = Rational { num: 30000, den: 1001 };

    #[test]
    fn from_seconds_rounds_to_nearest_tick() {
        // 1.0000166s at 30000 ticks/s = 30000.5 ticks -> rounds to 30001? No:
        // 1.00005s * 30000 = 30001.5 -> 30002; pick unambiguous cases:
        assert_eq!(Rational::from_seconds(1.0, 30000), Rational::new(30000, 30000));
        assert_eq!(Rational::from_seconds(0.5, 1000), Rational::new(500, 1000));
        assert_eq!(Rational::from_seconds(4.21, 1000), Rational::new(4210, 1000));
    }

    #[test]
    fn add_same_den_preserves_den() {
        let a = Rational::new(1001, 30000);
        let b = Rational::new(2002, 30000);
        assert_eq!(a.checked_add(b), Some(Rational::new(3003, 30000)));
    }

    #[test]
    fn add_mixed_den_uses_lcm() {
        let a = Rational::new(1, 2);
        let b = Rational::new(1, 3);
        assert_eq!(a.checked_add(b), Some(Rational::new(5, 6)));
    }

    #[test]
    fn sub_can_go_negative() {
        let a = Rational::new(1, 10);
        let b = Rational::new(3, 10);
        assert_eq!(a.checked_sub(b), Some(Rational::new(-2, 10)));
    }

    #[test]
    fn ordering_is_cross_denominator() {
        assert!(Rational::new(1, 3) < Rational::new(1, 2));
        assert!(Rational::new(30000, 30000) == Rational::new(1000, 1000));
        assert!(Rational::new(-1, 2) < Rational::new(0, 5));
    }

    #[test]
    fn rescale_rounds_to_nearest() {
        // 1/3 s at den 1000 = 333.33 ticks -> 333
        assert_eq!(Rational::new(1, 3).rescale(1000), Rational::new(333, 1000));
        // 2/3 s -> 666.67 -> 667
        assert_eq!(Rational::new(2, 3).rescale(1000), Rational::new(667, 1000));
    }

    #[test]
    fn snap_to_frame_ntsc() {
        // 0.5s at 30000/1001 fps: 0.5*30000/1001 = 14.985 frames -> frame 15
        // -> 15*1001/30000 s = 0.5005 s; in den 1000 that's 500.5 -> 501 ticks
        let t = Rational::new(500, 1000);
        assert_eq!(t.snap_to_frame(NTSC), Rational::new(501, 1000));
    }

    proptest! {
        #[test]
        fn from_seconds_to_secs_round_trip(secs in 0.0f64..36_000.0, tb in prop::sample::select(vec![1000u32, 30000, 24000, 60000, 90000])) {
            let r = Rational::from_seconds(secs, tb);
            let back = r.to_secs_f64();
            // within half a tick
            prop_assert!((back - secs).abs() <= 0.5 / tb as f64 + 1e-9);
        }

        #[test]
        fn add_then_sub_is_identity(n1 in -1_000_000i64..1_000_000, n2 in -1_000_000i64..1_000_000, d in prop::sample::select(vec![1000u32, 30000, 1001])) {
            let a = Rational::new(n1, d);
            let b = Rational::new(n2, d);
            let sum = a.checked_add(b).unwrap();
            prop_assert_eq!(sum.checked_sub(b).unwrap(), a);
            prop_assert_eq!(sum.den, d); // den preserved
        }
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p katto-engine rational`
Expected: compile errors — `new`, `from_seconds`, `checked_add`, … not found.

- [ ] **Step 4: Implement**

```rust
impl Rational {
    /// Construct a rational; `den` must be non-zero (all call sites pass fixed timebases).
    pub const fn new(num: i64, den: u32) -> Self {
        Self { num, den }
    }

    /// Decimal seconds -> nearest tick of `timebase` (ticks per second).
    /// Non-finite input maps to zero ticks; upstream validation rejects it earlier.
    pub fn from_seconds(secs: f64, timebase: u32) -> Self {
        let ticks = secs * f64::from(timebase);
        let num = if ticks.is_finite() { ticks.round() as i64 } else { 0 };
        Self { num, den: timebase }
    }

    /// Projection to display/model seconds. Boundary use only.
    pub fn to_secs_f64(self) -> f64 {
        self.num as f64 / f64::from(self.den)
    }

    /// Convert to a new timebase, rounding to the nearest tick (exact integer math).
    pub fn rescale(self, den: u32) -> Self {
        let num = div_round_nearest(i128::from(self.num) * i128::from(den), i128::from(self.den));
        Self { num: num as i64, den }
    }

    /// `self + rhs`; same-den fast path preserves the den, mixed dens use the lcm.
    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        combine(self, rhs, i128::checked_add)
    }

    /// `self - rhs`; same rules as [`Rational::checked_add`].
    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        combine(self, rhs, i128::checked_sub)
    }

    /// Scale the numerator by an integer factor; den preserved.
    pub fn checked_mul_int(self, k: i64) -> Option<Self> {
        let num = self.num.checked_mul(k)?;
        Some(Self { num, den: self.den })
    }

    /// Snap to the nearest integer frame of `fps`, returned in `self`'s timebase.
    pub fn snap_to_frame(self, fps: Rational) -> Self {
        let frame = div_round_nearest(
            i128::from(self.num) * i128::from(fps.num),
            i128::from(self.den) * i128::from(fps.den),
        );
        // frame * fps.den / fps.num seconds, back in self.den ticks:
        let num = div_round_nearest(
            frame * i128::from(fps.den) * i128::from(self.den),
            i128::from(fps.num),
        );
        Self { num: num as i64, den: self.den }
    }
}

impl PartialOrd for Rational {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Rational {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let lhs = i128::from(self.num) * i128::from(other.den);
        let rhs = i128::from(other.num) * i128::from(self.den);
        lhs.cmp(&rhs)
    }
}

/// Round-half-away-from-zero integer division.
fn div_round_nearest(num: i128, den: i128) -> i128 {
    debug_assert!(den > 0);
    if num >= 0 { (num + den / 2) / den } else { (num - den / 2) / den }
}

fn combine(a: Rational, b: Rational, op: fn(i128, i128) -> Option<i128>) -> Option<Rational> {
    if a.den == b.den {
        let num = op(i128::from(a.num), i128::from(b.num))?;
        return Some(Rational { num: i64::try_from(num).ok()?, den: a.den });
    }
    let g = gcd(u64::from(a.den), u64::from(b.den));
    let lcm = u64::from(a.den) / g * u64::from(b.den);
    let den = u32::try_from(lcm).ok()?;
    let an = i128::from(a.num) * i128::from(lcm / u64::from(a.den));
    let bn = i128::from(b.num) * i128::from(lcm / u64::from(b.den));
    let num = op(an, bn)?;
    Some(Rational { num: i64::try_from(num).ok()?, den })
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}
```

Note: `Eq`/`Hash` are already derived on the struct; the derived `PartialEq` is
**structural** (`1/2 != 2/4`), which is correct for timebase-preserving values — the tests
compare same-den values except the ordering test, which uses `==` via `Ord`… it doesn't:
`Rational::new(30000, 30000) == Rational::new(1000, 1000)` would fail structurally. Fix
that assertion to use `.cmp(..) == Ordering::Equal`:

```rust
assert_eq!(Rational::new(30000, 30000).cmp(&Rational::new(1000, 1000)), std::cmp::Ordering::Equal);
```

Every pub item gets a one-sentence `///` (crate has `#![warn(missing_docs)]`).

- [ ] **Step 5: Run tests**

Run: `cargo test -p katto-engine rational`
Expected: all PASS (including proptest).

- [ ] **Step 6: Commit**

```bash
git add crates/katto-engine/src/rational.rs crates/katto-engine/Cargo.toml Cargo.lock
git commit -m "feat(engine): complete Rational arithmetic, ordering, and frame snap"
```

---

### Task 2: Rational timing from ffprobe (`ProbeTiming`)

**Files:**
- Modify: `crates/katto-engine/src/ffprobe.rs`
- Create: `crates/katto-engine/tests/fixtures/ffprobe.zv-e10-4k60.json`
- Create: `crates/katto-engine/tests/fixtures/ffprobe.ntsc-2997df.json`

**Interfaces:**
- Consumes: `Rational` (Task 1).
- Produces: `pub struct ProbeTiming { pub frame_rate: Option<Rational>, pub duration: Option<Rational> }`
  and `pub fn parse_probe_timing(json: &str) -> Result<ProbeTiming>`.
  `MediaInfo`/`parse_probe` stay untouched (Phase-3 ingest consumes them; `duration_s`
  remains display-only).

Derivation rules (the hard constraint, encoded):
1. `frame_rate`: first video stream's `r_frame_rate` (e.g. `"30000/1001"`), parsed exactly
   — reuse the existing private `parse_ratio`. **Not** `avg_frame_rate` (PRD lock; known
   divergence from hyper-frames).
2. `duration`: first video stream's `duration_ts` (integer) × `time_base` (string
   `"num/den"`): `Rational { num: duration_ts * tb_num, den: tb_den }` (i128-checked).
3. Fallback when `duration_ts`/`time_base` missing: the stream's `duration` **string**
   (e.g. `"128.128000"`) parsed as an exact decimal — digits → `num`, `den = 10^places`
   (places clamped to ≤ 9, truncating further digits). Then `format.duration` the same way.
   **Never** route through `f64`.

- [ ] **Step 1: Capture fixtures.** Author two realistic ffprobe JSON files (hand-write
  from the documented ffprobe shape — keys `streams[].{codec_type, r_frame_rate,
  avg_frame_rate, time_base, duration_ts, duration}` and `format.duration`):
  - `ffprobe.zv-e10-4k60.json`: video stream `codec_type:"video"`, `r_frame_rate:"60000/1001"`,
    `avg_frame_rate:"60000/1001"`, `time_base:"1/60000"`, `duration_ts:7687680`,
    `duration:"128.128000"`; an audio stream; `format.duration:"128.150000"`.
  - `ffprobe.ntsc-2997df.json`: `r_frame_rate:"30000/1001"`, `time_base:"1/30000"`,
    `duration_ts:3843840`, `duration:"128.128000"`.
  If a real camera clip is on disk (check `~/Movies` or any project folder for a `.MP4`
  from the ZV-E10 II), prefer capturing real output:
  `ffprobe -loglevel error -print_format json -show_streams -show_format <clip> > fixture.json`.

- [ ] **Step 2: Write the failing tests** (in `ffprobe.rs` `mod tests`):

```rust
#[test]
fn timing_prefers_duration_ts_times_time_base() {
    let json = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/ffprobe.ntsc-2997df.json"),
    )
    .unwrap();
    let t = parse_probe_timing(&json).unwrap();
    assert_eq!(t.frame_rate, Some(Rational::new(30000, 1001)));
    assert_eq!(t.duration, Some(Rational::new(3_843_840, 30000)));
}

#[test]
fn timing_falls_back_to_exact_decimal_stream_duration() {
    let json = r#"{"streams":[{"codec_type":"video","r_frame_rate":"25/1","duration":"12.500000"}],"format":{}}"#;
    let t = parse_probe_timing(json).unwrap();
    assert_eq!(t.duration, Some(Rational::new(12_500_000, 1_000_000)));
}

#[test]
fn timing_never_uses_float_format_duration_field_of_media_info() {
    // format.duration decimal parsed exactly, den = 10^places
    let json = r#"{"streams":[{"codec_type":"video","r_frame_rate":"25/1"}],"format":{"duration":"3.5"}}"#;
    let t = parse_probe_timing(json).unwrap();
    assert_eq!(t.duration, Some(Rational::new(35, 10)));
}
```

- [ ] **Step 3: Run to verify failure** — `cargo test -p katto-engine ffprobe` → compile error.

- [ ] **Step 4: Implement** in `ffprobe.rs`:

```rust
/// Exact timing metadata for cut math: frame rate from `r_frame_rate` and duration
/// derived as `duration_ts x time_base` (never the float `duration_s`, which is
/// display-only).
#[derive(Debug, Clone, PartialEq)]
pub struct ProbeTiming {
    /// First video stream's `r_frame_rate` as an exact ratio.
    pub frame_rate: Option<Rational>,
    /// Source duration as an exact ratio in the stream's own timebase.
    pub duration: Option<Rational>,
}

/// Parse ffprobe JSON into exact [`ProbeTiming`].
///
/// # Errors
/// Returns [`Error::Probe`] when `json` is not valid JSON; missing fields degrade to `None`.
pub fn parse_probe_timing(json: &str) -> Result<ProbeTiming> {
    let root: Value = serde_json::from_str(json).map_err(|e| Error::Probe(e.to_string()))?;
    let streams = root.get("streams").and_then(Value::as_array);
    let video = streams.and_then(|s| {
        s.iter()
            .find(|st| st.get("codec_type").and_then(Value::as_str) == Some("video"))
    });

    let frame_rate = video
        .and_then(|v| v.get("r_frame_rate"))
        .and_then(Value::as_str)
        .and_then(parse_ratio);

    let duration = video
        .and_then(duration_from_ts)
        .or_else(|| {
            video
                .and_then(|v| v.get("duration"))
                .and_then(Value::as_str)
                .and_then(parse_exact_decimal)
        })
        .or_else(|| {
            root.get("format")
                .and_then(|f| f.get("duration"))
                .and_then(Value::as_str)
                .and_then(parse_exact_decimal)
        });

    Ok(ProbeTiming { frame_rate, duration })
}

fn duration_from_ts(stream: &Value) -> Option<Rational> {
    let ts = stream.get("duration_ts").and_then(Value::as_i64)?;
    let tb = stream.get("time_base").and_then(Value::as_str)?;
    let (tb_num, tb_den) = tb.split_once('/')?.into();
    // parse both sides
    let tb_num: i64 = tb_num.trim().parse().ok()?;
    let tb_den: u32 = tb_den.trim().parse().ok()?;
    let num = i128::from(ts).checked_mul(i128::from(tb_num))?;
    Some(Rational::new(i64::try_from(num).ok()?, tb_den))
}

/// Parse a decimal string like "128.128000" exactly: num over 10^places (places <= 9).
fn parse_exact_decimal(s: &str) -> Option<Rational> {
    let s = s.trim();
    let (int_part, frac_part) = s.split_once('.').unwrap_or((s, ""));
    let frac = &frac_part[..frac_part.len().min(9)];
    let den = 10u32.checked_pow(frac.len() as u32)?;
    let int_val: i64 = int_part.parse().ok()?;
    let frac_val: i64 = if frac.is_empty() { 0 } else { frac.parse().ok()? };
    let num = int_val.checked_mul(i64::from(den))?.checked_add(if int_val < 0 { -frac_val } else { frac_val })?;
    Some(Rational::new(num, den))
}
```

(`.into()` on a `split_once` tuple doesn't destructure — write it as
`let (tb_num_s, tb_den_s) = tb.split_once('/')?;` then parse each. The compiler will
catch this; fix accordingly.)

- [ ] **Step 5: Run** — `cargo test -p katto-engine ffprobe` → PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/katto-engine/src/ffprobe.rs crates/katto-engine/tests/fixtures/ffprobe.*.json
git commit -m "feat(engine): exact Rational probe timing from duration_ts x time_base"
```

---

### Task 3: Real schema types — transcript + wire cuts

**Files:**
- Modify: `crates/katto-engine/src/schema.rs` (becomes parent module: `pub mod cuts; pub mod edits; pub mod manifest; pub mod transcript;` + re-exports; edits/manifest submodules land in Tasks 5/6 — declare only what exists per task)
- Create: `crates/katto-engine/src/schema/transcript.rs`
- Create: `crates/katto-engine/src/schema/cuts.rs`
- Copy: `skills/clean-audio/scripts/__tests__/__fixtures__/transcript.valid.json`, `cuts.valid.json`, `cuts.overlap.json`, `cuts.out-of-bounds.json` → `crates/katto-engine/tests/fixtures/`
- Modify: `crates/katto-engine/src/lib.rs` (re-export surface)

**Interfaces:**
- Produces (used by validate/merge/bundle/planner/CLI/app):

```rust
// schema/transcript.rs — Scribe v2 shape, decimal seconds (model boundary)
pub struct Transcript {
    pub audio_duration_secs: Option<f64>,
    pub language_code: String,
    pub language_probability: f64,
    pub text: String,
    pub words: Vec<WordEntry>,
}
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WordEntry {
    Word { text: String, start: f64, end: f64, logprob: Option<f64>, speaker_id: Option<String> },
    Spacing { text: String, start: f64, end: f64 },
    AudioEvent { text: String, start: f64, end: f64 },
}
impl WordEntry {
    pub fn start(&self) -> f64; pub fn end(&self) -> f64; pub fn text(&self) -> &str;
}

// schema/cuts.rs — wire cuts.json, decimal seconds (model boundary), all fields pub
pub enum CutReason { Filler, Stutter, FalseStart, SelfCorrection, LongSilence, AudioEvent }   // serde snake_case
pub enum DiscretionaryReason { /* CutReason variants */ , Other }                              // serde snake_case
pub enum FlagReason { LowConfidence }                                                          // serde snake_case
pub enum Confidence { Low, Medium, High }                                                      // serde snake_case
pub struct Cut { pub start: f64, pub end: f64, pub reason: CutReason, pub excerpt: String }
pub struct Discretionary { pub start: f64, pub end: f64, pub reason: DiscretionaryReason,
                           pub excerpt: String, pub note: String, pub confidence: Confidence }
pub struct Flag { pub start: f64, pub end: f64, pub reason: FlagReason, pub excerpt: String, pub logprob: f64 }
pub struct Cuts {
    pub source_duration_secs: f64,
    pub cuts: Vec<Cut>,
    #[serde(default)] pub discretionary: Vec<Discretionary>,
    #[serde(default)] pub flags: Vec<Flag>,
    pub total_cut_secs: f64,
}
```

All types derive `Debug, Clone, PartialEq, Serialize, Deserialize`; enums additionally
`Copy, Eq, Hash`. Field names on the wire are exactly the PRD contract (snake_case —
**delete** the stub's `#[serde(rename_all = "camelCase")]`; the old stub types
(`Project`, private-field `Cuts`/`Edits`) are replaced wholesale). This intentionally
replaces the "mirrored verbatim" stub: katto's canonical shape is the clean-audio /
cut-decider contract restated normatively in `prd/phase-4.md`.

- [ ] **Step 1: Copy fixtures**

```bash
mkdir -p crates/katto-engine/tests/fixtures
cp skills/clean-audio/scripts/__tests__/__fixtures__/transcript.valid.json crates/katto-engine/tests/fixtures/
cp skills/clean-audio/scripts/__tests__/__fixtures__/cuts.valid.json crates/katto-engine/tests/fixtures/
cp skills/clean-audio/scripts/__tests__/__fixtures__/cuts.overlap.json crates/katto-engine/tests/fixtures/
cp skills/clean-audio/scripts/__tests__/__fixtures__/cuts.out-of-bounds.json crates/katto-engine/tests/fixtures/
```

- [ ] **Step 2: Write failing round-trip tests** (in `schema/cuts.rs` and
  `schema/transcript.rs` `mod tests`):

```rust
// transcript.rs tests
fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn parses_scribe_v2_fixture() {
    let t: Transcript = serde_json::from_str(&fixture("transcript.valid.json")).unwrap();
    assert!(t.words.iter().any(|w| matches!(w, WordEntry::Word { .. })));
    assert!(t.audio_duration_secs.is_some());
}

#[test]
fn word_entry_accessors_cover_all_variants() {
    let spacing: WordEntry = serde_json::from_str(r#"{"text":" ","type":"spacing","start":0.34,"end":0.47}"#).unwrap();
    assert_eq!(spacing.start(), 0.34);
    assert_eq!(spacing.text(), " ");
}

// cuts.rs tests
#[test]
fn parses_cuts_valid_fixture_with_defaulted_discretionary() {
    let c: Cuts = serde_json::from_str(&fixture("cuts.valid.json")).unwrap();
    assert_eq!(c.cuts.len(), 2);
    assert!(c.discretionary.is_empty()); // key absent in fixture -> default
    assert_eq!(c.cuts[0].reason, CutReason::Filler);
    assert_eq!(c.flags[0].reason, FlagReason::LowConfidence);
}

#[test]
fn reason_enums_serialize_snake_case() {
    assert_eq!(serde_json::to_string(&CutReason::FalseStart).unwrap(), "\"false_start\"");
    assert_eq!(serde_json::to_string(&DiscretionaryReason::Other).unwrap(), "\"other\"");
    assert_eq!(serde_json::to_string(&Confidence::Medium).unwrap(), "\"medium\"");
}
```

- [ ] **Step 3: Run to verify failure** — `cargo test -p katto-engine schema` → compile errors.

- [ ] **Step 4: Implement** the types exactly as in the Interfaces block, with `///` docs
  on every pub item. `schema.rs` becomes:

```rust
//! Serde schemas for the cut-pipeline artifact files. Wire shapes (`cuts.json`,
//! `transcript.json`) are decimal seconds at the model boundary; engine-domain
//! Rational projections live in `merge.rs`.

pub mod cuts;
pub mod transcript;

pub use cuts::{Confidence, Cut, CutReason, Cuts, Discretionary, DiscretionaryReason, Flag, FlagReason};
pub use transcript::{Transcript, WordEntry};
```

`lib.rs` keeps `pub mod schema;` (already present) — no glob re-exports.

- [ ] **Step 5: Run** — `cargo test -p katto-engine schema` → PASS. Also
  `cargo test -p katto-engine` (nothing else referenced the stub types; if the compiler
  disagrees, fix call sites in the same commit).

- [ ] **Step 6: Commit**

```bash
git add crates/katto-engine/src/schema.rs crates/katto-engine/src/schema/ crates/katto-engine/tests/fixtures/
git commit -m "feat(engine): real transcript and cuts wire schemas with ported fixtures"
```

---

### Task 4: cuts.json validation (`validate.rs`)

**Files:**
- Create: `crates/katto-engine/src/validate.rs`
- Create: `crates/katto-engine/tests/fixtures/cuts.misaligned-token.json`
- Create: `crates/katto-engine/tests/fixtures/cuts.flag-overlap.json`
- Create: `crates/katto-engine/tests/fixtures/cuts.bad-total.json`
- Modify: `crates/katto-engine/src/lib.rs` (`pub mod validate;`)

**Interfaces:**
- Consumes: `schema::{Cuts, Transcript, WordEntry}`.
- Produces:

```rust
pub const FLOAT_TOLERANCE: f64 = 1e-3;

#[derive(Debug, Clone, PartialEq)]
pub enum SpanList { Cuts, Discretionary, Flags }

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ValidationError {
    #[error("{list:?}[{index}]: start {start} must be >= 0 and < end {end}, end <= source duration {duration}")]
    OutOfBounds { list: SpanList, index: usize, start: f64, end: f64, duration: f64 },
    #[error("cuts[{index}]: not sorted by start (previous start {prev_start}, this start {start})")]
    Unsorted { index: usize, prev_start: f64, start: f64 },
    #[error("cuts[{index}]: overlaps previous cut (previous end {prev_end}, this start {start})")]
    Overlap { index: usize, prev_end: f64, start: f64 },
    #[error("discretionary[{d_index}]: overlaps cuts[{c_index}]")]
    DiscretionaryOverlapsCut { d_index: usize, c_index: usize },
    #[error("flags[{f_index}]: shares a span with cuts[{c_index}] — flagged words are never cut")]
    FlagSharesCutSpan { f_index: usize, c_index: usize },
    #[error("discretionary[{index}]: note must be non-empty")]
    EmptyNote { index: usize },
    #[error("total_cut_secs {stated} does not match sum of cut durations {computed:.6}")]
    TotalMismatch { stated: f64, computed: f64 },
    #[error("{list:?}[{index}]: boundary {value} is not a token boundary in the transcript")]
    MisalignedBoundary { list: SpanList, index: usize, value: f64 },
    #[error("{list:?}[{index}]: boundary {value} falls inside word token {token:?}")]
    InsideWordToken { list: SpanList, index: usize, value: f64, token: String },
}

/// Check every cuts.json invariant against its transcript. Empty vec == valid.
pub fn validate_cuts(cuts: &Cuts, transcript: &Transcript) -> Vec<ValidationError>;
```

Invariant mapping (PRD normative list; all comparisons with `FLOAT_TOLERANCE`):
1. bounds → `OutOfBounds` for every list (`0 ≤ start < end ≤ source_duration_secs`).
2. cuts sorted (`Unsorted`) + non-overlap (`Overlap`, `cuts[i].end ≤ cuts[i+1].start`).
3. discretionary vs cuts overlap → `DiscretionaryOverlapsCut` (interval overlap test
   `d.start < c.end - tol && c.start < d.end - tol`, same as the Zod reference).
4. flags never share a span with cuts → `FlagSharesCutSpan` (overlap test, not just exact
   key match — stricter than the Zod reference, per PRD "never shares a span").
5. flag `logprob` presence + discretionary `confidence` validity are enforced by serde at
   parse time (required fields / closed enums); `EmptyNote` covers the non-empty-note half.
6. `TotalMismatch`: `|Σ(cut.end - cut.start) - total_cut_secs| ≤ 0.001` (cuts only).
7. Token alignment for **cuts and discretionary** (not flags): each `start`/`end` must
   equal some token's `start` or `end` (any type) within tolerance → else
   `MisalignedBoundary`; and must not fall strictly inside a `Word` token's interval
   (`w.start + tol < b < w.end - tol`) → `InsideWordToken`.

- [ ] **Step 1: Author the three new fixtures** as siblings of `cuts.valid.json`, each a
  minimal delta from it (read `transcript.valid.json` first and pick real token boundaries
  from it; the invalid fixtures then break exactly one invariant each):
  - `cuts.misaligned-token.json`: one cut whose `start` is offset +0.02 s from any token
    boundary (and not inside a word — that's `MisalignedBoundary`, not `InsideWordToken`).
  - `cuts.flag-overlap.json`: a flag whose span overlaps a cut span.
  - `cuts.bad-total.json`: `total_cut_secs` off by 0.05.

  **Fixture-pairing check:** the ported `cuts.valid.json` was validated by the Zod schema,
  which has no token-alignment invariant. Load it against `transcript.valid.json`; if any
  boundary misaligns, adjust `cuts.valid.json`'s times to real token boundaries from the
  transcript (they are our fixtures now; keep the same shape/reasons).

- [ ] **Step 2: Write the failing table-driven test** (in `validate.rs` `mod tests`):

```rust
use rstest::rstest;

fn load(name: &str) -> Cuts {
    serde_json::from_str(&std::fs::read_to_string(format!(
        "{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR")
    )).unwrap()).unwrap()
}

fn transcript() -> Transcript {
    serde_json::from_str(&std::fs::read_to_string(format!(
        "{}/tests/fixtures/transcript.valid.json", env!("CARGO_MANIFEST_DIR")
    )).unwrap()).unwrap()
}

#[test]
fn valid_fixture_passes() {
    assert_eq!(validate_cuts(&load("cuts.valid.json"), &transcript()), vec![]);
}

#[rstest]
#[case("cuts.overlap.json")]
#[case("cuts.out-of-bounds.json")]
#[case("cuts.misaligned-token.json")]
#[case("cuts.flag-overlap.json")]
#[case("cuts.bad-total.json")]
fn invalid_fixture_names_the_invariant(#[case] name: &str) {
    let errors = validate_cuts(&load(name), &transcript());
    assert!(!errors.is_empty(), "{name} should fail validation");
    // each error's Display names the offending entry
    for e in &errors {
        assert!(!e.to_string().is_empty());
    }
}

#[test]
fn overlap_fixture_reports_overlap_variant() {
    let errors = validate_cuts(&load("cuts.overlap.json"), &transcript());
    assert!(errors.iter().any(|e| matches!(e, ValidationError::Overlap { .. })));
}

#[test]
fn bad_total_reports_total_mismatch() {
    let errors = validate_cuts(&load("cuts.bad-total.json"), &transcript());
    assert!(errors.iter().any(|e| matches!(e, ValidationError::TotalMismatch { .. })));
}

#[test]
fn boundary_inside_word_token_is_named() {
    // synthesize: transcript with one word 1.0..2.0; a cut 1.5..2.0 starts inside it
    let t: Transcript = serde_json::from_str(r#"{
        "audio_duration_secs": 3.0, "language_code": "en", "language_probability": 1.0,
        "text": "hi",
        "words": [{"text":"hi","type":"word","start":1.0,"end":2.0,"logprob":-0.1}]
    }"#).unwrap();
    let c: Cuts = serde_json::from_str(r#"{
        "source_duration_secs": 3.0,
        "cuts": [{"start":1.5,"end":2.0,"reason":"filler","excerpt":"hi"}],
        "flags": [], "total_cut_secs": 0.5
    }"#).unwrap();
    let errors = validate_cuts(&c, &t);
    assert!(errors.iter().any(|e| matches!(e, ValidationError::InsideWordToken { .. })));
}
```

- [ ] **Step 3: Run to verify failure** — `cargo test -p katto-engine validate` → compile error.

- [ ] **Step 4: Implement `validate_cuts`.** Structure:

```rust
pub fn validate_cuts(cuts: &Cuts, transcript: &Transcript) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let dur = cuts.source_duration_secs;

    check_bounds(&mut errors, SpanList::Cuts, cuts.cuts.iter().map(|c| (c.start, c.end)), dur);
    check_bounds(&mut errors, SpanList::Discretionary, cuts.discretionary.iter().map(|d| (d.start, d.end)), dur);
    check_bounds(&mut errors, SpanList::Flags, cuts.flags.iter().map(|f| (f.start, f.end)), dur);

    // 2. sorted + non-overlapping cuts (in given order — order is part of the contract)
    for i in 1..cuts.cuts.len() {
        let (prev, cur) = (&cuts.cuts[i - 1], &cuts.cuts[i]);
        if cur.start < prev.start - FLOAT_TOLERANCE {
            errors.push(ValidationError::Unsorted { index: i, prev_start: prev.start, start: cur.start });
        } else if cur.start < prev.end - FLOAT_TOLERANCE {
            errors.push(ValidationError::Overlap { index: i, prev_end: prev.end, start: cur.start });
        }
    }
    // 3, 4: overlap sweeps; 5: EmptyNote; 6: TotalMismatch; 7: token alignment.
    // Token boundary set: collect every entry's start and end (sorted Vec<f64>);
    // is_boundary(b) = binary-search window |cand - b| <= FLOAT_TOLERANCE.
    // inside_word(b) = any Word entry with start + tol < b < end - tol.
    errors
}
```

Write the helpers (`check_bounds`, `overlaps(a, b) -> bool`, `token_boundaries`,
`check_alignment`) as private fns in the same file. Iterate exactly the invariant list
above — each invariant is independently checked (do not early-return).

- [ ] **Step 5: Run** — `cargo test -p katto-engine validate` → PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/katto-engine/src/validate.rs crates/katto-engine/src/lib.rs crates/katto-engine/tests/fixtures/cuts.*.json
git commit -m "feat(engine): cuts.json invariant validation with typed errors"
```

---

### Task 5: Rational projection + edits schema + cuts↔edits merge

**Files:**
- Create: `crates/katto-engine/src/schema/edits.rs`
- Create: `crates/katto-engine/src/merge.rs`
- Modify: `crates/katto-engine/src/schema.rs` (add `pub mod edits;` + re-exports)
- Modify: `crates/katto-engine/src/lib.rs` (`pub mod merge;`)

**Interfaces:**
- Consumes: `Rational` (Task 1), `schema::cuts` (Task 3).
- Produces:

```rust
// schema/edits.rs — katto-owned wire format (snake_case keys), Rational-native
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CutEdge { Start, End }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundaryAdjustment { pub cut_index: usize, pub edge: CutEdge, pub new_time: Rational }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManualCut { pub start: Rational, pub end: Rational, pub note: Option<String> }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Edits {
    pub schema_version: u32,                    // 1
    #[serde(default)] pub toggled_off: Vec<usize>,          // indices into cuts.json cuts[]
    #[serde(default)] pub applied_discretionary: Vec<usize>, // indices into discretionary[]
    #[serde(default)] pub manual_cuts: Vec<ManualCut>,
    #[serde(default)] pub boundary_adjustments: Vec<BoundaryAdjustment>,
}

// merge.rs — engine domain (Rational, converted exactly once)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CutSource {
    Base { index: usize },
    Discretionary { index: usize },
    Manual { index: usize },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectiveCut { pub start: Rational, pub end: Rational, pub source: CutSource }

#[derive(Debug, Clone, PartialEq)]
pub struct CutPlan { pub timebase: u32, pub base: Vec<(Rational, Rational)>, pub discretionary: Vec<(Rational, Rational)> }

impl CutPlan {
    /// THE float->Rational boundary: convert wire cuts to engine Rational exactly once,
    /// in the video frame timebase (ticks per second).
    pub fn from_wire(cuts: &Cuts, timebase: u32) -> CutPlan;
}

/// Base cuts minus toggled-off, plus applied discretionary, plus manual cuts, with
/// boundary adjustments applied to base cuts; sorted by start. Pure and deterministic.
pub fn effective_cuts(plan: &CutPlan, edits: &Edits) -> Vec<EffectiveCut>;
```

Merge semantics (exhaustively tested; deterministic):
1. Start from `plan.base`, skipping indices in `edits.toggled_off`.
2. Apply each `BoundaryAdjustment` to its base cut (`cut_index` refers to the **original**
   cuts.json index, valid even if later toggled — an adjustment on a toggled-off cut is a
   no-op in the output). `new_time` is rescaled to `plan.timebase` before use.
3. Append `plan.discretionary[i]` for each `i` in `applied_discretionary`.
4. Append `manual_cuts` (rescaled to `plan.timebase`); their `source` index is their
   position in `edits.manual_cuts`.
5. Drop any span where `end <= start` after adjustment (an adjustment can invert a cut —
   inverted cuts vanish rather than error; the UI prevents this in Phase 5).
6. Sort by `start` (then `end`). **No coalescing** — overlaps are preserved so each span
   keeps its provenance; the Phase-5 renderer coalesces at apply time. Out-of-range
   indices (toggle/apply/adjust pointing past the arrays) are ignored, not errors —
   edits.json can outlive a re-plan.

- [ ] **Step 1: Write the failing tests** (in `merge.rs` `mod tests`) — the PRD's four
  exhaustive cases plus determinism:

```rust
fn wire(cuts: &[(f64, f64)], disc: &[(f64, f64)]) -> Cuts {
    Cuts {
        source_duration_secs: 100.0,
        cuts: cuts.iter().map(|&(s, e)| Cut { start: s, end: e, reason: CutReason::Filler, excerpt: String::new() }).collect(),
        discretionary: disc.iter().map(|&(s, e)| Discretionary { start: s, end: e, reason: DiscretionaryReason::Other, excerpt: String::new(), note: "n".into(), confidence: Confidence::Medium }).collect(),
        flags: vec![],
        total_cut_secs: cuts.iter().map(|(s, e)| e - s).sum(),
    }
}

const TB: u32 = 1000;

#[test]
fn from_wire_converts_once_at_given_timebase() {
    let plan = CutPlan::from_wire(&wire(&[(4.21, 4.68)], &[]), TB);
    assert_eq!(plan.base, vec![(Rational::new(4210, TB), Rational::new(4680, TB))]);
}

#[test]
fn toggle_off_removes_base_cut() {
    let plan = CutPlan::from_wire(&wire(&[(1.0, 2.0), (3.0, 4.0)], &[]), TB);
    let edits = Edits { toggled_off: vec![0], ..Default::default() };
    let out = effective_cuts(&plan, &edits);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].source, CutSource::Base { index: 1 });
}

#[test]
fn applied_discretionary_joins_sorted() {
    let plan = CutPlan::from_wire(&wire(&[(3.0, 4.0)], &[(1.0, 2.0)]), TB);
    let edits = Edits { applied_discretionary: vec![0], ..Default::default() };
    let out = effective_cuts(&plan, &edits);
    assert_eq!(out[0].source, CutSource::Discretionary { index: 0 });
    assert_eq!(out[1].source, CutSource::Base { index: 0 });
}

#[test]
fn manual_cut_may_overlap_toggled_off_span() {
    let plan = CutPlan::from_wire(&wire(&[(1.0, 2.0)], &[]), TB);
    let edits = Edits {
        toggled_off: vec![0],
        manual_cuts: vec![ManualCut { start: Rational::new(1500, TB), end: Rational::new(2500, TB), note: None }],
        ..Default::default()
    };
    let out = effective_cuts(&plan, &edits);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].source, CutSource::Manual { index: 0 });
    assert_eq!(out[0].start, Rational::new(1500, TB));
}

#[test]
fn boundary_adjustment_moves_base_edge() {
    let plan = CutPlan::from_wire(&wire(&[(1.0, 2.0)], &[]), TB);
    let edits = Edits {
        boundary_adjustments: vec![BoundaryAdjustment { cut_index: 0, edge: CutEdge::End, new_time: Rational::new(1800, TB) }],
        ..Default::default()
    };
    let out = effective_cuts(&plan, &edits);
    assert_eq!(out[0].end, Rational::new(1800, TB));
}

#[test]
fn inverting_adjustment_drops_the_cut() {
    let plan = CutPlan::from_wire(&wire(&[(1.0, 2.0)], &[]), TB);
    let edits = Edits {
        boundary_adjustments: vec![BoundaryAdjustment { cut_index: 0, edge: CutEdge::End, new_time: Rational::new(500, TB) }],
        ..Default::default()
    };
    assert!(effective_cuts(&plan, &edits).is_empty());
}

#[test]
fn out_of_range_indices_are_ignored() {
    let plan = CutPlan::from_wire(&wire(&[(1.0, 2.0)], &[]), TB);
    let edits = Edits { toggled_off: vec![7], applied_discretionary: vec![3], ..Default::default() };
    assert_eq!(effective_cuts(&plan, &edits).len(), 1);
}

#[test]
fn empty_edits_is_identity_on_base() {
    let plan = CutPlan::from_wire(&wire(&[(1.0, 2.0), (3.0, 4.0)], &[(5.0, 6.0)]), TB);
    let out = effective_cuts(&plan, &Edits::default());
    assert_eq!(out.len(), 2); // discretionary not applied by default
}
```

Plus an edits.json round-trip test in `schema/edits.rs`:

```rust
#[test]
fn edits_round_trip_preserves_rational() {
    let e = Edits {
        schema_version: 1,
        toggled_off: vec![2],
        applied_discretionary: vec![],
        manual_cuts: vec![ManualCut { start: Rational::new(1, 30000), end: Rational::new(2, 30000), note: None }],
        boundary_adjustments: vec![BoundaryAdjustment { cut_index: 0, edge: CutEdge::Start, new_time: Rational::new(5, 1000) }],
    };
    let json = serde_json::to_string(&e).unwrap();
    assert_eq!(serde_json::from_str::<Edits>(&json).unwrap(), e);
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p katto-engine merge` → compile errors.

- [ ] **Step 3: Implement** `schema/edits.rs` and `merge.rs` per the Interfaces block.
  `from_wire` uses `Rational::from_seconds(x, timebase)` — the **only** place wire floats
  become Rational. `effective_cuts` is ~40 lines of straightforward vec building + sort by
  `(start, end)` using the `Ord` from Task 1.

- [ ] **Step 4: Run** — `cargo test -p katto-engine merge && cargo test -p katto-engine edits` → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/katto-engine/src/schema/edits.rs crates/katto-engine/src/merge.rs crates/katto-engine/src/schema.rs crates/katto-engine/src/lib.rs
git commit -m "feat(engine): edits schema and deterministic cuts-edits merge"
```

---

### Task 6: `.kruproj` bundle (`manifest` + `bundle.rs`)

**Files:**
- Create: `crates/katto-engine/src/schema/manifest.rs`
- Create: `crates/katto-engine/src/bundle.rs`
- Modify: `crates/katto-engine/src/schema.rs`, `crates/katto-engine/src/lib.rs`
- Modify: `crates/katto-engine/src/error.rs`

**Interfaces:**
- Consumes: `Rational`, `schema::{Cuts, Edits, Transcript}`.
- Produces:

```rust
// schema/manifest.rs — project.json (snake_case)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub schema_version: u32,                 // 1
    pub source_video_absolute_path: PathBuf,
    pub frame_rate: Rational,
    pub duration: Rational,
}

// bundle.rs
pub const PROJECT_JSON: &str = "project.json";
pub const TRANSCRIPT_JSON: &str = "transcript.json";
pub const CUTS_JSON: &str = "cuts.json";
pub const EDITS_JSON: &str = "edits.json";
pub const CACHED_AUDIO_WAV: &str = "cached_audio.wav";

#[derive(Debug, Clone, PartialEq)]
pub struct Bundle {
    pub root: PathBuf,
    pub manifest: ProjectManifest,
    pub transcript: Option<Transcript>,
    pub cuts: Option<Cuts>,
    pub edits: Option<Edits>,
}

/// Atomically write `bytes` to `path` via `<path>.tmp` -> rename.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()>;

/// Serialize `value` as pretty JSON and write atomically.
pub fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()>;

/// Open a bundle directory; missing optional artifacts are None; a missing source video
/// yields Error::SourceMissing (typed; relocation is Phase 5).
pub fn open(root: &Path) -> Result<Bundle>;

/// Open without checking the source video exists (pipeline-internal steps).
pub fn open_unchecked(root: &Path) -> Result<Bundle>;

/// Write edits.json atomically (Phase 5 calls this on auto-save; round-trip tested now).
pub fn save_edits(root: &Path, edits: &Edits) -> Result<()>;
```

- Error variants added to `error.rs` (same enum, same style as `Probe`):

```rust
    /// Filesystem failure in bundle/import handling.
    #[error("io: {0}")]
    Io(String),
    /// Bundle artifact malformed or missing where required.
    #[error("bundle: {0}")]
    Bundle(String),
    /// The manifest's source video is missing on open (Phase 5 adds relocation).
    #[error("source missing: expected {expected_path}")]
    SourceMissing { expected_path: PathBuf, filename: String, duration: Rational },
```

plus `impl From<std::io::Error> for Error` mapping to `Io`.

`schema.rs` gains `pub mod manifest;` **and** `pub use manifest::ProjectManifest;` so the
type is addressable as `katto_engine::schema::ProjectManifest` (Tasks 7, 13, and 15 use
that path).

- [ ] **Step 1: Write the failing round-trip test** (in `bundle.rs` `mod tests`, using
  `tempfile` — add `tempfile = "3"` to engine dev-deps):

```rust
fn manifest(source: &Path) -> ProjectManifest {
    ProjectManifest {
        schema_version: 1,
        source_video_absolute_path: source.to_path_buf(),
        frame_rate: Rational::new(30000, 1001),
        duration: Rational::new(3_843_840, 30000),
    }
}

#[test]
fn bundle_round_trip_open_mutate_save_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("clip.mp4");
    std::fs::write(&source, b"fake").unwrap();
    let root = dir.path().join("clip.kruproj");
    std::fs::create_dir(&root).unwrap();
    write_json_atomic(&root.join(PROJECT_JSON), &manifest(&source)).unwrap();

    let mut b = open(&root).unwrap();
    assert!(b.transcript.is_none() && b.cuts.is_none() && b.edits.is_none());

    let edits = Edits { schema_version: 1, toggled_off: vec![0], ..Default::default() };
    save_edits(&root, &edits).unwrap();
    b = open(&root).unwrap();
    assert_eq!(b.edits, Some(edits));
    // no .tmp litter
    assert!(!root.join("edits.json.tmp").exists());
}

#[test]
fn missing_source_is_typed() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("clip.kruproj");
    std::fs::create_dir(&root).unwrap();
    write_json_atomic(&root.join(PROJECT_JSON), &manifest(&dir.path().join("gone.mp4"))).unwrap();
    match open(&root) {
        Err(Error::SourceMissing { filename, .. }) => assert_eq!(filename, "gone.mp4"),
        other => panic!("expected SourceMissing, got {other:?}"),
    }
    // unchecked open still succeeds for pipeline-internal steps
    assert!(open_unchecked(&root).is_ok());
}

#[test]
fn write_atomic_replaces_not_appends() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("f.json");
    write_atomic(&p, b"first-longer-content").unwrap();
    write_atomic(&p, b"second").unwrap();
    assert_eq!(std::fs::read(&p).unwrap(), b"second");
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p katto-engine bundle` → compile errors.

- [ ] **Step 3: Implement.** `write_atomic`: write to
  `path.with_extension(format!("{}.tmp", ext))` — simpler and collision-safe: sibling file
  named `<file_name>.tmp` — then `std::fs::rename`. `open`: read+parse `project.json`
  (missing/unparseable → `Error::Bundle`), each optional artifact parsed when present
  (parse failure of a present file is `Error::Bundle`, not silent `None`), then check
  `manifest.source_video_absolute_path.exists()` → else `SourceMissing` with `filename` =
  file_name lossy string and `duration` from the manifest.

- [ ] **Step 4: Run** — `cargo test -p katto-engine bundle` → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/katto-engine/src/schema/manifest.rs crates/katto-engine/src/bundle.rs \
        crates/katto-engine/src/schema.rs crates/katto-engine/src/lib.rs \
        crates/katto-engine/src/error.rs crates/katto-engine/Cargo.toml Cargo.lock
git commit -m "feat(engine): kruproj bundle open/save with atomic writes and typed SourceMissing"
```

---

### Task 7: `import` — probe + audio extraction into a bundle

**Files:**
- Create: `crates/katto-engine/src/import.rs`
- Modify: `crates/katto-engine/src/lib.rs`, `crates/katto-engine/Cargo.toml`

**Interfaces:**
- Consumes: `ffprobe::parse_probe_timing` (Task 2), `bundle::{write_json_atomic, PROJECT_JSON, CACHED_AUDIO_WAV}` (Task 6).
- Produces:

```rust
/// Argv for probing (identical flags to the app crate's Phase-3 probe).
pub fn ffprobe_argv(path: &Path) -> Vec<String>;
/// Argv for mono 16 kHz WAV extraction into the bundle (PRD: `-vn -ar 16000 -ac 1`).
pub fn extract_audio_argv(src: &Path, out: &Path) -> Vec<String>;

#[derive(Debug, Clone, PartialEq)]
pub struct ImportOutcome { pub bundle_root: PathBuf, pub manifest: ProjectManifest }

/// Probe `video`, create `<parent>/<basename>.kruproj/` with project.json, extract
/// cached_audio.wav. `parent` is the project's `audio/` dir (or any dir for loose bundles).
pub async fn import(video: &Path, parent: &Path) -> Result<ImportOutcome>;
```

Engine `Cargo.toml` additions (this task):

```toml
tokio = { workspace = true, features = ["process", "io-util", "time", "fs", "macros"] }

[dev-dependencies]
# (existing) + :
tempfile = "3"
tokio = { workspace = true, features = ["rt-multi-thread", "macros"] }
```

`extract_audio_argv` (deterministic, pure — mirrors hyper-frames' pinned style but WAV per
PRD):

```
ffmpeg -nostdin -loglevel error -y -i <src> -vn -ac 1 -ar 16000 -c:a pcm_s16le <out>
```

`import` behavior: spawn `ffprobe` (`tokio::process::Command`, capture stdout; non-zero
exit → `Error::Probe` with stderr text), `parse_probe_timing`; missing `frame_rate` or
`duration` → `Error::Probe("no video timing …")`. Create bundle dir
(`create_dir_all`), write `project.json` atomically (source stored **absolute** via
`std::fs::canonicalize`), then spawn `ffmpeg` writing to `cached_audio.wav.tmp` and rename
on success (stderr surfaced in the error; bundle state stays intact — project.json already
written is fine, a partial `.tmp` is removed on failure). If the bundle dir already exists,
re-probe and overwrite project.json + cached_audio.wav (idempotent re-import).

- [ ] **Step 1: Write failing pure tests** (argv builders) in `import.rs` `mod tests`:

```rust
#[test]
fn extract_audio_argv_is_pinned() {
    let argv = extract_audio_argv(Path::new("/a/clip.mp4"), Path::new("/b/cached_audio.wav.tmp"));
    assert_eq!(argv, vec![
        "ffmpeg", "-nostdin", "-loglevel", "error", "-y",
        "-i", "/a/clip.mp4",
        "-vn", "-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le",
        "/b/cached_audio.wav.tmp",
    ]);
}

#[test]
fn ffprobe_argv_matches_phase3_flags() {
    let argv = ffprobe_argv(Path::new("/a/clip.mp4"));
    assert_eq!(argv, vec![
        "-loglevel", "error", "-print_format", "json", "-show_streams", "-show_format",
        "/a/clip.mp4",
    ]);
}
```

Plus the ignored hardware-adjacent integration test (dev machine has ffmpeg):

```rust
#[tokio::test]
#[ignore = "spawns real ffmpeg/ffprobe; run manually with a real clip"]
async fn import_real_clip_end_to_end() {
    let clip = std::env::var("KATTO_TEST_CLIP").expect("set KATTO_TEST_CLIP=/path/to/clip.mp4");
    let dir = tempfile::tempdir().unwrap();
    let out = import(Path::new(&clip), dir.path()).await.unwrap();
    assert!(out.bundle_root.join(CACHED_AUDIO_WAV).exists());
    assert!(out.manifest.duration.num > 0);
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p katto-engine import` → compile errors.

- [ ] **Step 3: Implement** per the behavior spec. The two spawn call-sites stay thin
  (`run_capturing(cmd, argv) -> Result<String /*stdout*/>` private helper; per
  `.claude/rules/testing.md` they are not unit-tested).

- [ ] **Step 4: Run** — `cargo test -p katto-engine import` (pure tests PASS; ignored test
  skipped). Optionally run the ignored test once with a real clip if one is present.

- [ ] **Step 5: Commit**

```bash
git add crates/katto-engine/src/import.rs crates/katto-engine/src/lib.rs crates/katto-engine/Cargo.toml Cargo.lock
git commit -m "feat(engine): import step - probe timing plus cached audio extraction"
```

---

### Task 8: `transcribe` — ElevenLabs Scribe v2 client

**Files:**
- Create: `crates/katto-engine/src/transcribe.rs`
- Modify: `crates/katto-engine/src/lib.rs`, `error.rs`, `Cargo.toml`

**Interfaces:**
- Consumes: `schema::Transcript`, `bundle::write_atomic`.
- Produces:

```rust
pub const ELEVENLABS_BASE_URL: &str = "https://api.elevenlabs.io";

#[derive(Debug, Clone)]
pub struct TranscribeConfig {
    pub api_key: String,          // never logged; Debug impl redacts (hand-write Debug)
    pub base_url: String,         // injectable for tests
}

/// POST cached_audio.wav to Scribe v2; returns the raw response body plus the parsed
/// transcript. One automatic retry on 429/5xx after 5 s.
pub async fn transcribe(cfg: &TranscribeConfig, wav_path: &Path) -> Result<(Vec<u8>, Transcript)>;

/// Run transcribe and atomically persist the raw body as transcript.json in the bundle.
pub async fn transcribe_into_bundle(cfg: &TranscribeConfig, bundle_root: &Path) -> Result<Transcript>;
```

- Error variants added:

```rust
    /// ElevenLabs rejected the API key (401).
    #[error("elevenlabs auth: {0}")]
    TranscribeAuth(String),
    /// ElevenLabs quota/rate limit (429) after retry.
    #[error("elevenlabs quota: {0}")]
    TranscribeQuota(String),
    /// Any other transcription transport/response failure.
    #[error("elevenlabs: {0}")]
    Transcribe(String),
```

Engine `Cargo.toml` additions:

```toml
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "multipart", "json"] }

[dev-dependencies]
# + :
wiremock = "0.6"
```

Request (from the verified contract): multipart form with `file` part
(`file_name("cached_audio.wav")`, `mime_str("audio/wav")`), text parts `model_id` =
`scribe_v2`, `timestamps_granularity` = `word`, `diarize` = `true`, `tag_audio_events` =
`true`; header `xi-api-key`. Response handling: 200 → parse bytes into `Transcript`
(parse failure → `Error::Transcribe` including serde message); a body containing
`"transcripts"` (multichannel) or `"request_id"` (webhook) at the top level →
`Error::Transcribe("unexpected multichannel/webhook response")`; 401 → `TranscribeAuth`;
429 → sleep 5 s, retry once, then `TranscribeQuota`; 5xx → retry once then `Transcribe`;
422 → `Transcribe` with body text. `transcribe_into_bundle` writes the **raw body**
atomically (full response stored, PRD) only after successful parse — no partial writes.
`audio_duration_secs: None` in the parsed transcript → `Error::Transcribe("missing
audio_duration_secs")` (cuts validation needs it; same policy as the clean-audio
reference).

- [ ] **Step 1: Write failing wiremock tests** (in `transcribe.rs` `mod tests`):

```rust
use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

fn transcript_body() -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/transcript.valid.json", env!("CARGO_MANIFEST_DIR")
    )).unwrap()
}

async fn wav_file(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let p = dir.path().join("cached_audio.wav");
    std::fs::write(&p, b"RIFFfakewav").unwrap();
    p
}

#[tokio::test]
async fn posts_multipart_and_parses_transcript() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/v1/speech-to-text"))
        .and(matchers::header("xi-api-key", "k123"))
        .respond_with(ResponseTemplate::new(200).set_body_string(transcript_body()))
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let cfg = TranscribeConfig { api_key: "k123".into(), base_url: server.uri() };
    let (raw, t) = transcribe(&cfg, &wav_file(&dir).await).await.unwrap();
    assert!(!raw.is_empty());
    assert!(t.audio_duration_secs.is_some());
}

#[tokio::test]
async fn auth_failure_is_typed() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(401).set_body_string("bad key"))
        .mount(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let cfg = TranscribeConfig { api_key: "bad".into(), base_url: server.uri() };
    assert!(matches!(
        transcribe(&cfg, &wav_file(&dir).await).await,
        Err(Error::TranscribeAuth(_))
    ));
}

#[tokio::test]
async fn quota_retries_once_then_types() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(429).set_body_string("slow down"))
        .expect(2) // initial + one retry
        .mount(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let cfg = TranscribeConfig { api_key: "k".into(), base_url: server.uri() };
    assert!(matches!(
        transcribe(&cfg, &wav_file(&dir).await).await,
        Err(Error::TranscribeQuota(_))
    ));
}

#[tokio::test]
async fn into_bundle_writes_raw_body_atomically() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string(transcript_body()))
        .mount(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("b.kruproj");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("cached_audio.wav"), b"RIFF").unwrap();
    let cfg = TranscribeConfig { api_key: "k".into(), base_url: server.uri() };
    transcribe_into_bundle(&cfg, &root).await.unwrap();
    let on_disk = std::fs::read_to_string(root.join("transcript.json")).unwrap();
    assert_eq!(on_disk, transcript_body()); // raw body verbatim
    assert!(!root.join("transcript.json.tmp").exists());
}
```

(Retry sleep: gate the 5 s on `#[cfg(not(test))]` — in tests use 10 ms — via a private
`const RETRY_DELAY: Duration` chosen with `cfg!(test)`.)

- [ ] **Step 2: Run to verify failure** — `cargo test -p katto-engine transcribe` → compile errors.

- [ ] **Step 3: Implement** per spec. Hand-write `Debug` for `TranscribeConfig` printing
  `api_key: "<redacted>"`. Read the wav with `tokio::fs::read` (file sizes here are tens
  of MB — acceptable to buffer; Scribe cap is 5 GB, ours are ~115 MB/hour of footage).

- [ ] **Step 4: Run** — `cargo test -p katto-engine transcribe` → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/katto-engine/src/transcribe.rs crates/katto-engine/src/lib.rs \
        crates/katto-engine/src/error.rs crates/katto-engine/Cargo.toml Cargo.lock
git commit -m "feat(engine): ElevenLabs Scribe v2 client with typed retryable errors"
```

---

### Task 9: Planner foundation — prompt, trait, parsers

**Files:**
- Create: `crates/katto-engine/prompts/cut-decider.md`
- Create: `crates/katto-engine/src/planner.rs`
- Create: `crates/katto-engine/src/planner/partial.rs`
- Modify: `crates/katto-engine/src/lib.rs`, `error.rs`

**Interfaces:**
- Consumes: `schema::{Cuts, Cut, Transcript}`, `validate::ValidationError`.
- Produces:

```rust
// planner.rs
/// The committed cut-decider system prompt (external input, body verbatim).
pub const CUT_DECIDER_PROMPT: &str = include_str!("../prompts/cut-decider.md");

/// Appended to the system prompt in both planner modes: overrides the file's
/// Write-tool output discipline for a toolless, single-shot reply.
pub const OUTPUT_OVERRIDE: &str = "\n\n## Runtime override (katto)\n\nYou are running as a single-shot planner with NO tools. Do not attempt to read or write any file. Reply with ONLY the cuts.json JSON object as plain text - no prose, no markdown fences, no commentary.\n";

#[derive(Debug, thiserror::Error)]
pub enum PlanError {
    #[error("claude subprocess: {0}")]
    Subprocess(String),
    #[error("anthropic api: {0}")]
    Http(String),
    #[error("anthropic auth: {0}")]
    Auth(String),
    #[error("planner returned invalid output: {error}")]
    Invalid { error: String, raw: String },
    /// Second failure: surfaced with the raw output as a debugging aid (PRD).
    #[error("planner output invalid after retry: {error}\n--- raw planner output ---\n{raw}")]
    InvalidAfterRetry { error: String, raw: String },
}

/// Single-shot cut planner (D14: no agent loop).
pub trait CutPlanner {
    /// Plan cuts for `transcript`; implementations run the validate-retry-once loop.
    fn plan(&self, transcript: &Transcript)
        -> impl std::future::Future<Output = std::result::Result<Cuts, PlanError>> + Send;
}

/// Parse planner reply text into `Cuts`, tolerating markdown fences / leading prose by
/// extracting the first balanced top-level JSON object.
pub fn parse_cuts_json(raw: &str) -> std::result::Result<Cuts, PlanError>;

/// Render validation errors into the retry correction message (PRD wording).
pub fn correction_message(errors: &[ValidationError]) -> String;
// -> "the JSON you returned was invalid: <error lines>; return only valid JSON matching the schema"

// planner/partial.rs
/// Extract the complete `Cut` objects from a *prefix* of planner output text — used to
/// stream cuts into the UI while the model is still emitting. Never errors; incomplete
/// tails yield what is parseable so far.
pub fn cuts_prefix(text: &str) -> Vec<Cut>;
```

- Error enum gains: `#[error("plan: {0}")] Plan(#[from] planner::PlanError),` and
  `#[error("cuts validation failed: {0}")] CutsInvalid(String),` (used by CLI/app when
  re-validating on load).

- [ ] **Step 1: Port the prompt.**

```bash
mkdir -p crates/katto-engine/prompts
```

Copy `agents/cut-decider.md` → `crates/katto-engine/prompts/cut-decider.md` **stripping
only the YAML frontmatter block** (`---` … `---`, lines 1–8: name/description/model/
effort/color/tools — harness metadata, not prompt body). Body from `# cut-decider — …`
onward is verbatim, unmodified. (The file's "Output discipline" section mentions the
Write tool; `OUTPUT_OVERRIDE` — appended at runtime, never edited into the file —
supersedes it.)

- [ ] **Step 2: Write failing tests** (in `planner.rs` and `planner/partial.rs` `mod tests`):

```rust
// planner.rs tests
#[test]
fn prompt_is_embedded_and_frontmatter_free() {
    assert!(CUT_DECIDER_PROMPT.starts_with("# cut-decider"));
    assert!(CUT_DECIDER_PROMPT.contains("Cut Policy"));
    assert!(!CUT_DECIDER_PROMPT.contains("model:"));
}

#[test]
fn parse_accepts_bare_json() {
    let raw = std::fs::read_to_string(format!(
        "{}/tests/fixtures/cuts.valid.json", env!("CARGO_MANIFEST_DIR")
    )).unwrap();
    assert!(parse_cuts_json(&raw).is_ok());
}

#[test]
fn parse_strips_fences_and_prose() {
    let raw = format!(
        "Here is the plan:\n```json\n{}\n```\nDone.",
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/cuts.valid.json", env!("CARGO_MANIFEST_DIR")
        )).unwrap()
    );
    assert!(parse_cuts_json(&raw).is_ok());
}

#[test]
fn parse_failure_carries_raw() {
    match parse_cuts_json("not json at all") {
        Err(PlanError::Invalid { raw, .. }) => assert_eq!(raw, "not json at all"),
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn correction_message_uses_prd_wording() {
    let errs = vec![ValidationError::TotalMismatch { stated: 1.0, computed: 2.0 }];
    let msg = correction_message(&errs);
    assert!(msg.starts_with("the JSON you returned was invalid: "));
    assert!(msg.ends_with("; return only valid JSON matching the schema"));
}

// planner/partial.rs tests
#[test]
fn empty_prefix_yields_nothing() {
    assert!(cuts_prefix("").is_empty());
    assert!(cuts_prefix("{\"source_duration_secs\": 12.0, \"cuts\": [").is_empty());
}

#[test]
fn complete_objects_in_incomplete_array_are_extracted() {
    let prefix = r#"{"source_duration_secs": 12.0, "cuts": [
        {"start": 1.0, "end": 1.5, "reason": "filler", "excerpt": "um"},
        {"start": 3.0, "end": 3.5, "reason": "stutter", "excerpt": "I-I"},
        {"start": 5.0, "end"#;
    let cuts = cuts_prefix(prefix);
    assert_eq!(cuts.len(), 2);
    assert_eq!(cuts[1].excerpt, "I-I");
}

#[test]
fn full_document_extracts_all_cuts() {
    let raw = std::fs::read_to_string(format!(
        "{}/tests/fixtures/cuts.valid.json", env!("CARGO_MANIFEST_DIR")
    )).unwrap();
    assert_eq!(cuts_prefix(&raw).len(), 2);
}
```

- [ ] **Step 3: Run to verify failure** — `cargo test -p katto-engine planner` → compile errors.

- [ ] **Step 4: Implement.**
  - `parse_cuts_json`: find the first `{`, walk a small state machine (depth counter,
    in-string flag, escape flag) to the matching `}`, `serde_json::from_str` that slice;
    any failure → `PlanError::Invalid { error, raw: raw.to_owned() }`.
  - `cuts_prefix`: locate `"cuts"` then the following `[`; iterate balanced `{…}` slices
    with the same state machine; `serde_json::from_str::<Cut>` each complete slice,
    stopping at the first incomplete/unparseable tail.
  - `correction_message`: join error `Display`s with `"; "` inside the PRD sentence.

- [ ] **Step 5: Run** — `cargo test -p katto-engine planner` → PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/katto-engine/prompts/cut-decider.md crates/katto-engine/src/planner.rs \
        crates/katto-engine/src/planner/ crates/katto-engine/src/lib.rs crates/katto-engine/src/error.rs
git commit -m "feat(engine): planner trait, committed cut-decider prompt, tolerant parsers"
```

---

### Task 10: `SubprocessClaudePlanner`

**Files:**
- Create: `crates/katto-engine/src/planner/subprocess.rs`
- Create: `crates/katto-engine/tests/fixtures/claude.stream-json.txt`
- Modify: `crates/katto-engine/src/planner.rs` (`pub mod subprocess;` etc.)

**Interfaces:**
- Consumes: `CUT_DECIDER_PROMPT`, `OUTPUT_OVERRIDE`, `PlanError`, `partial::cuts_prefix`,
  retry loop (Task 12 wires `plan`; this task builds the raw attempt layer).
- Produces:

```rust
/// Streaming observer for incremental cut arrival (app wires this to Channel events).
pub trait PartialObserver: Send + Sync {
    /// Called whenever the count of parseable cuts in the accumulating reply grows.
    fn on_cuts(&self, cuts: &[Cut]);
}

pub struct SubprocessClaudePlanner {
    pub claude_path: PathBuf,
    pub workdir: PathBuf,             // --resume is directory-scoped; keep both calls here
    pub observer: Option<Arc<dyn PartialObserver>>,
    pub timeout: Duration,            // default 10 min
}

impl SubprocessClaudePlanner {
    pub fn new(claude_path: PathBuf, workdir: PathBuf) -> Self;
    pub fn with_observer(self, obs: Arc<dyn PartialObserver>) -> Self;
}

/// Pure argv builders (unit-tested; spawn site stays thin).
pub fn first_attempt_argv() -> Vec<String>;
// ["-p", "--output-format", "stream-json", "--verbose", "--include-partial-messages",
//  "--system-prompt", <CUT_DECIDER_PROMPT + OUTPUT_OVERRIDE>]
pub fn correction_argv(session_id: &str) -> Vec<String>;
// ["-p", "--output-format", "json", "--resume", <session_id>]

/// Accumulates NDJSON stream lines into reply text + final envelope. Pure.
#[derive(Debug, Default)]
pub struct StreamAccum { /* text: String, final_: Option<FinalEnvelope> */ }
#[derive(Debug, Clone, PartialEq)]
pub struct FinalEnvelope { pub text: String, pub session_id: Option<String>, pub is_error: bool }
impl StreamAccum {
    /// Feed one NDJSON line; returns true when visible reply text grew.
    pub fn push_line(&mut self, line: &str) -> bool;
    pub fn text(&self) -> &str;
    pub fn finish(self) -> Option<FinalEnvelope>;
}

pub(crate) struct RawAttempt { pub text: String, pub session_id: Option<String> }
impl SubprocessClaudePlanner {
    pub(crate) async fn first(&self, transcript_json: &str) -> Result<RawAttempt, PlanError>;
    pub(crate) async fn correction(&self, session_id: Option<&str>, transcript_json: &str, message: &str) -> Result<RawAttempt, PlanError>;
}
```

Stream-line handling (from the verified headless docs): a line is a JSON object; lines
with `type == "stream_event"` whose `event.delta.type == "text_delta"` contribute
`event.delta.text` to the accumulating reply (only when `parent_tool_use_id` is null/absent
— main conversation only); the line with `type == "result"` carries `result` (full text —
**authoritative**, replaces the accumulation), `session_id`, `is_error`. Unknown line
shapes are ignored (forward-compatible). On each growth, the spawn loop calls
`cuts_prefix(accum.text())` and notifies the observer when the count increases.

Spawn behavior (`first`): `tokio::process::Command::new(&self.claude_path)`,
`current_dir(&self.workdir)`, args from `first_attempt_argv`, stdin piped (write
`transcript_json`, then drop stdin to close it), stdout read line-by-line
(`tokio::io::BufReader::lines`), the whole call wrapped in `tokio::time::timeout`.
Non-zero exit or `is_error: true` → `PlanError::Subprocess(stderr + envelope text)` (the
app maps this to "offer API-key mode" messaging). `correction`: when `session_id` is
`Some`, spawn with `correction_argv` and stdin = the correction message; when `None`
(session id missing from the envelope), fall back to a fresh `first`-style call with stdin
= transcript + `"\n\n"` + correction message. Never pass `--bare` (auth), never pass
`--model` (subscription default).

- [ ] **Step 1: Author the stream fixture** `claude.stream-json.txt` — hand-written NDJSON
  matching the documented shapes (one line each): a `system/init` line, three
  `stream_event` text-delta lines that together spell a small cuts.json (split
  mid-object so the partial extractor is exercised), and a final `result` line:

```
{"type":"system","subtype":"init","session_id":"sess-1"}
{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"{\"source_duration_secs\": 8.0, \"cuts\": [{\"start\": 1.0, \"end\": 1.2, \"reason\": \"filler\", \"excerpt\": \"um\"},"}},"parent_tool_use_id":null,"session_id":"sess-1"}
{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"{\"start\": 4.0, \"end\": 4.5, \"reason\": \"audio_event\", \"excerpt\": \"[cough]\"}],"}},"parent_tool_use_id":null,"session_id":"sess-1"}
{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":" \"discretionary\": [], \"flags\": [], \"total_cut_secs\": 0.7}"}},"parent_tool_use_id":null,"session_id":"sess-1"}
{"type":"result","subtype":"success","is_error":false,"result":"{\"source_duration_secs\": 8.0, \"cuts\": [{\"start\": 1.0, \"end\": 1.2, \"reason\": \"filler\", \"excerpt\": \"um\"},{\"start\": 4.0, \"end\": 4.5, \"reason\": \"audio_event\", \"excerpt\": \"[cough]\"}], \"discretionary\": [], \"flags\": [], \"total_cut_secs\": 0.7}","session_id":"sess-1","total_cost_usd":0.01}
```

  **[CONFIRM-AT-IMPL]** The stream-event line shape (`event.delta` nesting,
  `parent_tool_use_id` location) is documented but not captured from a live run; before
  finalizing the fixture, run one cheap real call and eyeball the shapes:
  `echo "reply with the word ok" | claude -p --output-format stream-json --verbose --include-partial-messages | head -20`.
  Adjust fixture + parser to what the real CLI emits; keep the fixture as the frozen truth.

- [ ] **Step 2: Write failing tests**:

```rust
#[test]
fn argv_builders_are_exact() {
    let argv = first_attempt_argv();
    assert_eq!(argv[0], "-p");
    assert!(argv.contains(&"stream-json".to_string()));
    assert!(argv.contains(&"--include-partial-messages".to_string()));
    let sys = &argv[argv.iter().position(|a| a == "--system-prompt").unwrap() + 1];
    assert!(sys.starts_with("# cut-decider"));
    assert!(sys.ends_with(OUTPUT_OVERRIDE));
    assert!(!argv.contains(&"--bare".to_string())); // auth: never bare (D14)

    assert_eq!(correction_argv("sess-1"), vec![
        "-p", "--output-format", "json", "--resume", "sess-1",
    ]);
}

#[test]
fn stream_accum_replays_fixture() {
    let fixture = std::fs::read_to_string(format!(
        "{}/tests/fixtures/claude.stream-json.txt", env!("CARGO_MANIFEST_DIR")
    )).unwrap();
    let mut acc = StreamAccum::default();
    let mut growth_counts = Vec::new();
    for line in fixture.lines() {
        if acc.push_line(line) {
            growth_counts.push(crate::planner::partial::cuts_prefix(acc.text()).len());
        }
    }
    // first delta yields 1 complete cut, second completes the 2nd
    assert!(growth_counts.contains(&1));
    assert!(growth_counts.contains(&2));
    let fin = acc.finish().unwrap();
    assert_eq!(fin.session_id.as_deref(), Some("sess-1"));
    assert!(!fin.is_error);
    assert!(crate::planner::parse_cuts_json(&fin.text).is_ok());
}
```

- [ ] **Step 3: Run to verify failure** — `cargo test -p katto-engine subprocess` → compile errors.

- [ ] **Step 4: Implement** per spec. The spawn helpers are thin and untested-by-unit
  (covered later by an `#[ignore]`d test in Task 13 and by the owner checkpoint).

- [ ] **Step 5: Run** — `cargo test -p katto-engine subprocess` → PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/katto-engine/src/planner/subprocess.rs crates/katto-engine/src/planner.rs \
        crates/katto-engine/tests/fixtures/claude.stream-json.txt
git commit -m "feat(engine): subprocess claude planner with streamed partial cuts"
```

---

### Task 11: `HttpAnthropicPlanner`

**Files:**
- Create: `crates/katto-engine/src/planner/http.rs`
- Modify: `crates/katto-engine/src/planner.rs`

**Interfaces:**
- Produces:

```rust
pub const ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com";
pub const DEFAULT_MODEL: &str = "claude-sonnet-4-6";   // PRD-locked default, settings-overridable
pub const MAX_TOKENS: u32 = 8192;                      // PRD-locked

pub struct HttpAnthropicPlanner {
    pub api_key: String,       // redacted Debug, never logged
    pub model: String,
    pub base_url: String,      // injectable for tests
}

/// Pure request-body builder (unit-tested).
pub fn request_body(model: &str, system: &str, messages: &[(String, String)]) -> serde_json::Value;
// messages: (role, content) pairs; body = {model, max_tokens: 8192, system, messages:[{role, content}]}

pub(crate) struct HttpAttempt { pub text: String, pub messages: Vec<(String, String)> }
impl HttpAnthropicPlanner {
    pub(crate) async fn first(&self, transcript_json: &str) -> Result<HttpAttempt, PlanError>;
    // messages = [("user", transcript_json)]
    pub(crate) async fn correction(&self, prior: &HttpAttempt, message: &str) -> Result<HttpAttempt, PlanError>;
    // messages = prior.messages + [("assistant", prior.text), ("user", message)]
}
```

Response handling: 200 → `json.content[0].text` (missing → `PlanError::Http("empty
content")`); 401/403 → `PlanError::Auth`; anything else → `PlanError::Http` with the
error body's `error.message` when parseable. Headers: `x-api-key`,
`anthropic-version: 2023-06-01`, `content-type: application/json`. Non-streaming
(single-shot; incremental cut arrival is a subprocess-mode feature — documented decision:
the HTTP fallback emits its cuts in one batch when the response lands).

- [ ] **Step 1: Write failing tests**:

```rust
#[test]
fn request_body_matches_messages_api() {
    let body = request_body("claude-sonnet-4-6", "SYS", &[("user".into(), "T".into())]);
    assert_eq!(body["model"], "claude-sonnet-4-6");
    assert_eq!(body["max_tokens"], 8192);
    assert_eq!(body["system"], "SYS");
    assert_eq!(body["messages"][0]["role"], "user");
    assert_eq!(body["messages"][0]["content"], "T");
}

#[tokio::test]
async fn first_posts_and_extracts_text() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/v1/messages"))
        .and(wiremock::matchers::header("x-api-key", "sk-test"))
        .and(wiremock::matchers::header("anthropic-version", "2023-06-01"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": [{"type": "text", "text": "{\"ok\":1}"}],
            "stop_reason": "end_turn"
        })))
        .expect(1)
        .mount(&server).await;
    let p = HttpAnthropicPlanner { api_key: "sk-test".into(), model: DEFAULT_MODEL.into(), base_url: server.uri() };
    let a = p.first("TRANSCRIPT").await.unwrap();
    assert_eq!(a.text, "{\"ok\":1}");
    assert_eq!(a.messages, vec![("user".to_string(), "TRANSCRIPT".to_string())]);
}

#[tokio::test]
async fn auth_error_is_typed() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "type": "error", "error": {"type": "authentication_error", "message": "bad key"}
        })))
        .mount(&server).await;
    let p = HttpAnthropicPlanner { api_key: "bad".into(), model: DEFAULT_MODEL.into(), base_url: server.uri() };
    assert!(matches!(p.first("T").await, Err(PlanError::Auth(_))));
}

#[tokio::test]
async fn correction_appends_assistant_and_user_turns() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_partial_json(serde_json::json!({
            "messages": [
                {"role": "user", "content": "T"},
                {"role": "assistant", "content": "BAD"},
                {"role": "user", "content": "fix it"}
            ]
        })))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": [{"type": "text", "text": "GOOD"}]
        })))
        .expect(1)
        .mount(&server).await;
    let p = HttpAnthropicPlanner { api_key: "k".into(), model: DEFAULT_MODEL.into(), base_url: server.uri() };
    let prior = HttpAttempt { text: "BAD".into(), messages: vec![("user".into(), "T".into())] };
    let a = p.correction(&prior, "fix it").await.unwrap();
    assert_eq!(a.text, "GOOD");
}
```

- [ ] **Step 2: Run to verify failure**, **Step 3: Implement**, **Step 4: Run to PASS** —
  `cargo test -p katto-engine http`.

- [ ] **Step 5: Commit**

```bash
git add crates/katto-engine/src/planner/http.rs crates/katto-engine/src/planner.rs
git commit -m "feat(engine): Anthropic Messages API planner (BYOK fallback)"
```

---

### Task 12: Validate-retry loop (`planner/retry.rs`) + trait impls

**Files:**
- Create: `crates/katto-engine/src/planner/retry.rs`
- Modify: `crates/katto-engine/src/planner.rs`, `planner/subprocess.rs`, `planner/http.rs`

**Interfaces:**
- Produces:

```rust
// retry.rs — generic over an attempt driver so it's testable with stubs
pub(crate) trait AttemptDriver {
    type Attempt;
    async fn first(&self, transcript_json: &str) -> Result<(String, Self::Attempt), PlanError>;
    async fn correction(&self, prior: Self::Attempt, message: &str) -> Result<(String, Self::Attempt), PlanError>;
}

/// Parse + validate; on invalid JSON or invariant violation retry exactly once with the
/// correction message; second failure surfaces the raw output (PRD).
pub(crate) async fn plan_with_retry<D: AttemptDriver>(
    driver: &D,
    transcript: &Transcript,
) -> std::result::Result<Cuts, PlanError>;
```

Flow: serialize transcript (`serde_json::to_string`), `first` → `parse_cuts_json`; on
parse success run `validate_cuts(&cuts, transcript)`; empty → done. On parse failure use
its `PlanError::Invalid.error`; on validation failure use `correction_message(&errors)`.
Then `correction(prior, msg)` → parse+validate again; failure →
`PlanError::InvalidAfterRetry { error, raw }`. Transport errors (`Subprocess`, `Http`,
`Auth`) propagate immediately, no retry (they are the jobs layer's concern).

`impl CutPlanner for SubprocessClaudePlanner` and `for HttpAnthropicPlanner`: adapt their
`first`/`correction` to `AttemptDriver` (subprocess `Attempt = Option<String>` session id,
carrying the transcript for the resume-less fallback; http `Attempt = HttpAttempt`), then
`plan()` = `retry::plan_with_retry(self, transcript)`. Also add the runtime-selection
enum:

```rust
/// Runtime planner selection (claude detected -> Subprocess, else BYOK Http).
pub enum Planner {
    Subprocess(SubprocessClaudePlanner),
    Http(HttpAnthropicPlanner),
}
impl CutPlanner for Planner { /* delegate */ }
```

- [ ] **Step 1: Write failing stub tests** (in `retry.rs` `mod tests`):

```rust
struct ScriptedDriver { replies: std::sync::Mutex<Vec<String>>, corrections_seen: std::sync::Mutex<Vec<String>> }

impl AttemptDriver for ScriptedDriver {
    type Attempt = ();
    async fn first(&self, _t: &str) -> Result<(String, ()), PlanError> {
        Ok((self.replies.lock().unwrap().remove(0), ()))
    }
    async fn correction(&self, _p: (), msg: &str) -> Result<(String, ()), PlanError> {
        self.corrections_seen.lock().unwrap().push(msg.to_string());
        Ok((self.replies.lock().unwrap().remove(0), ()))
    }
}

fn valid_raw() -> String { /* read cuts.valid.json fixture */ }
fn transcript() -> Transcript { /* read transcript.valid.json fixture */ }

#[tokio::test]
async fn valid_first_attempt_needs_no_retry() {
    let d = ScriptedDriver { replies: vec![valid_raw()].into(), corrections_seen: vec![].into() };
    let cuts = plan_with_retry(&d, &transcript()).await.unwrap();
    assert_eq!(cuts.cuts.len(), 2);
    assert!(d.corrections_seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn invalid_then_valid_retries_once_with_prd_message() {
    let d = ScriptedDriver { replies: vec!["garbage".into(), valid_raw()].into(), corrections_seen: vec![].into() };
    assert!(plan_with_retry(&d, &transcript()).await.is_ok());
    let seen = d.corrections_seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert!(seen[0].starts_with("the JSON you returned was invalid: "));
}

#[tokio::test]
async fn invariant_violation_also_triggers_retry() {
    // bad-total fixture parses fine but fails validation
    let bad = std::fs::read_to_string(format!(
        "{}/tests/fixtures/cuts.bad-total.json", env!("CARGO_MANIFEST_DIR")
    )).unwrap();
    let d = ScriptedDriver { replies: vec![bad, valid_raw()].into(), corrections_seen: vec![].into() };
    assert!(plan_with_retry(&d, &transcript()).await.is_ok());
}

#[tokio::test]
async fn second_failure_surfaces_raw_output() {
    let d = ScriptedDriver { replies: vec!["garbage".into(), "still garbage".into()].into(), corrections_seen: vec![].into() };
    match plan_with_retry(&d, &transcript()).await {
        Err(PlanError::InvalidAfterRetry { raw, .. }) => assert_eq!(raw, "still garbage"),
        other => panic!("expected InvalidAfterRetry, got {other:?}"),
    }
}
```

(Async fns in traits used generically — no `async_trait` dep needed; if the borrow
checker fights the `Mutex<Vec<String>>` stub, switch to `RefCell` + `#[tokio::test]`
single-thread. Tests may use `unwrap`.)

- [ ] **Step 2: Run to verify failure**, **Step 3: Implement** retry + both trait impls +
  `Planner` enum, **Step 4: Run** — `cargo test -p katto-engine retry` and
  `cargo test -p katto-engine` (whole crate) → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/katto-engine/src/planner/
git commit -m "feat(engine): validate-retry-once planner loop and runtime planner selection"
```

---

### Task 13: Engine pipeline integration test

**Files:**
- Create: `crates/katto-engine/tests/pipeline.rs`
- Create: `crates/katto-engine/tests/fixtures/cuts.for-transcript-valid.json`
  (a valid cuts file whose boundaries align with `transcript.valid.json` tokens — may
  reuse the adjusted `cuts.valid.json` if Task 4's pairing check made them align; then
  skip this file and use `cuts.valid.json`)

**Interfaces:**
- Consumes: everything from Tasks 3–12 through the **pub API only** (integration test).

- [ ] **Step 1: Write the test** (fails until any missing pub re-export is added):

```rust
//! End-to-end: prepared bundle -> mocked ElevenLabs -> stub planner -> validated cuts on
//! disk. No network, no ffmpeg, no claude binary.

use katto_engine::bundle::{self, CUTS_JSON, PROJECT_JSON, TRANSCRIPT_JSON};
use katto_engine::planner::{parse_cuts_json, CutPlanner, PlanError};
use katto_engine::schema::{Cuts, Transcript};
use katto_engine::transcribe::{transcribe_into_bundle, TranscribeConfig};
use katto_engine::validate::validate_cuts;
use katto_engine::Rational;

struct CannedPlanner { raw: String }
impl CutPlanner for CannedPlanner {
    async fn plan(&self, t: &Transcript) -> Result<Cuts, PlanError> {
        let cuts = parse_cuts_json(&self.raw)?;
        assert!(validate_cuts(&cuts, t).is_empty());
        Ok(cuts)
    }
}

#[tokio::test]
async fn transcribe_then_plan_lands_validated_cuts_in_bundle() {
    // bundle scaffold
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("clip.mp4");
    std::fs::write(&source, b"fake").unwrap();
    let root = dir.path().join("clip.kruproj");
    std::fs::create_dir(&root).unwrap();
    bundle::write_json_atomic(&root.join(PROJECT_JSON), &katto_engine::schema::ProjectManifest {
        schema_version: 1,
        source_video_absolute_path: source.clone(),
        frame_rate: Rational::new(30000, 1001),
        duration: Rational::new(3_843_840, 30000),
    }).unwrap();
    std::fs::write(root.join("cached_audio.wav"), b"RIFF").unwrap();

    // mocked ElevenLabs
    let server = wiremock::MockServer::start().await;
    let transcript_raw = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/transcript.valid.json"
    )).unwrap();
    wiremock::Mock::given(wiremock::matchers::path("/v1/speech-to-text"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(transcript_raw))
        .mount(&server).await;
    let cfg = TranscribeConfig { api_key: "k".into(), base_url: server.uri() };
    let transcript = transcribe_into_bundle(&cfg, &root).await.unwrap();

    // stub planner (canned valid cuts) + persist
    let raw = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/cuts.valid.json"
    )).unwrap();
    let cuts = CannedPlanner { raw }.plan(&transcript).await.unwrap();
    bundle::write_json_atomic(&root.join(CUTS_JSON), &cuts).unwrap();

    // reopen: full bundle materializes
    let b = bundle::open(&root).unwrap();
    assert!(b.transcript.is_some());
    assert_eq!(b.cuts.unwrap().cuts.len(), 2);
    assert!(root.join(TRANSCRIPT_JSON).exists());
}
```

Add a second `#[ignore = "requires ffmpeg + claude + real keys; owner checkpoint"]` test
`full_pipeline_real_binaries` chaining `import` → `transcribe` (env
`ELEVENLABS_API_KEY`) → `SubprocessClaudePlanner` — body mirrors the above but with real
calls; it exists so the owner (or CI with `--features expensive-tests` later) can run the
true end-to-end. Guard: read env vars, `return` early with a message if unset.

- [ ] **Step 2: Run** — `cargo test -p katto-engine --test pipeline` → PASS (fix any
  missing pub re-exports in `lib.rs`/`schema.rs` — the test exercising only pub API is the
  point).

- [ ] **Step 3: Commit**

```bash
git add crates/katto-engine/tests/pipeline.rs crates/katto-engine/tests/fixtures/
git commit -m "test(engine): mocked end-to-end pipeline integration"
```

---

### Task 14: `katto-cli` — clap surface

**Files:**
- Modify: `crates/katto-cli/Cargo.toml`, `crates/katto-cli/src/main.rs`
- Create: `crates/katto-cli/src/cli.rs`, `src/keys.rs`, `src/output.rs`
- Create: `crates/katto-cli/tests/cli.rs`

**Interfaces:**
- Consumes: engine pub API (import, transcribe, planner, bundle, validate).
- Produces the command surface (PRD + Phase-5 slots):

```
katto import <video> [--project <dir>]     # bundle at <project>/audio/<basename>.kruproj,
                                           # or <video-parent>/<basename>.kruproj when no --project (loose bundle)
katto transcribe <bundle>
katto plan <bundle> [--planner subprocess|http] [--model <id>]
katto cut <video> [--project <dir>] [--planner ...]   # import + transcribe + plan
katto auth status
# Phase 5 will add: katto render <bundle>, katto export <bundle>  (slots reserved: the
# Command enum is exhaustive today; Phase 5 appends variants — no restructuring needed)
```

Global: `--json` flag (machine output). Exit codes: 0 ok, 1 pipeline error, 2 usage
(clap 4's parse-error default exit code is already 2 — keep it).

`Cargo.toml` additions:

```toml
[dependencies]
katto-engine = { path = "../katto-engine" }
anyhow = "1"                    # binary main only (rust.md)
clap = { version = "4.5", features = ["derive"] }
serde_json.workspace = true
tokio = { workspace = true, features = ["rt-multi-thread", "macros"] }
keyring-core = "1.0"

[target.'cfg(target_os = "macos")'.dependencies]
apple-native-keyring-store = { version = "1.0", features = ["keychain"] }

[dev-dependencies]
assert_cmd = "2"
insta = "1"
predicates = "3"
```

Key resolution (`keys.rs`) — PRD: "CLI uses the keychain via a small shared helper or env
var". Do both, env first:

```rust
pub enum KeySource { Env, Keychain, Missing }
/// ELEVENLABS_API_KEY / ANTHROPIC_API_KEY env var, else keychain service "katto",
/// account "elevenlabs"/"anthropic" (same constants as the app crate).
pub fn resolve(name: KeyName) -> (Option<String>, KeySource);
pub fn init_keychain();   // set apple-native store once; no-op off-macOS
```

Planner selection for `plan`/`cut`: `--planner` override; default = `which claude`
(`std::process::Command::new("zsh").args(["-lc", "which claude"])`, mirroring the app's
`detect_claude`) → subprocess; else Anthropic key present → http; else exit 1 with
"no planner available: install claude or store an Anthropic API key (katto auth status)".

`output.rs` holds **pure render functions** (`fn render_auth_status(&AuthStatus, json: bool) -> String`,
`fn render_import(&ImportOutcome, json: bool) -> String`, …) — snapshot-tested with insta;
`main.rs` stays a thin `#[tokio::main]` + `anyhow::Result<()>` that prints and maps errors
to exit code 1 (`std::process::exit(1)` after eprintln of the engine error `Display`).

- [ ] **Step 1: Write failing tests**
  - insta snapshots over the pure render fns (in `output.rs` `mod tests`):

```rust
#[test]
fn auth_status_human_render() {
    let s = AuthStatus {
        claude_path: Some("/Users/x/.local/bin/claude".into()),
        elevenlabs: KeySource::Keychain,
        anthropic: KeySource::Missing,
    };
    insta::assert_snapshot!("auth_status_human", render_auth_status(&s, false));
}

#[test]
fn auth_status_json_render() {
    let s = AuthStatus { claude_path: None, elevenlabs: KeySource::Env, anthropic: KeySource::Env };
    insta::assert_snapshot!("auth_status_json", render_auth_status(&s, true));
}
```

  - exit-code integration tests (`tests/cli.rs`, assert_cmd):

```rust
use assert_cmd::Command;

#[test]
fn no_args_is_usage_error_exit_2() {
    Command::cargo_bin("katto-cli").unwrap().assert().code(2);
}

#[test]
fn import_missing_file_is_pipeline_error_exit_1() {
    Command::cargo_bin("katto-cli").unwrap()
        .args(["import", "/nonexistent/clip.mp4"])
        .assert()
        .code(1)
        .stderr(predicates::str::is_empty().not());
}

#[test]
fn auth_status_exits_0() {
    Command::cargo_bin("katto-cli").unwrap()
        .args(["auth", "status", "--json"])
        .assert()
        .code(0);
}
```

  (Binary name: check `[[bin]]`/package name — the crate is `katto-cli`, so
  `cargo_bin("katto-cli")`; if the owner wants the installed binary named `katto`, add
  `[[bin]] name = "katto"` and update `cargo_bin` calls — do it now, it's the PRD's
  spelling: `katto cut …`. Use `[[bin]] name = "katto", path = "src/main.rs"`.)

- [ ] **Step 2: Run to verify failure** — `cargo test -p katto-cli` → compile errors.

- [ ] **Step 3: Implement.** `cli.rs` clap derive:

```rust
#[derive(clap::Parser)]
#[command(name = "katto", version, about = "katto cut pipeline CLI")]
pub struct Cli {
    /// Emit machine-readable JSON instead of human output.
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(clap::Subcommand)]
pub enum Command {
    /// Probe a clip and create its .kruproj bundle with extracted audio.
    Import { video: PathBuf, #[arg(long)] project: Option<PathBuf> },
    /// Transcribe a bundle's cached audio with ElevenLabs Scribe v2.
    Transcribe { bundle: PathBuf },
    /// Plan cuts for a transcribed bundle.
    Plan { bundle: PathBuf, #[arg(long)] planner: Option<PlannerChoice>, #[arg(long)] model: Option<String> },
    /// import + transcribe + plan in one shot.
    Cut { video: PathBuf, #[arg(long)] project: Option<PathBuf>, #[arg(long)] planner: Option<PlannerChoice> },
    /// Show claude detection and key presence.
    Auth { #[command(subcommand)] cmd: AuthCmd },
}

#[derive(clap::Subcommand)]
pub enum AuthCmd { Status }

#[derive(Clone, Copy, clap::ValueEnum)]
pub enum PlannerChoice { Subprocess, Http }
```

  Command bodies call engine fns directly (import → `engine::import::import`; transcribe →
  `transcribe_into_bundle` with resolved key; plan → open_unchecked bundle, require
  transcript, select planner, `plan()`, `write_json_atomic(cuts.json)`, print counts).
  `cut` chains the three. `--json` routes through the `output.rs` render fns. Run
  `cargo insta review`-free by using `insta::assert_snapshot!` and committing the
  generated `.snap` files (never `.snap.new`).

- [ ] **Step 4: Run** — `cargo test -p katto-cli` → PASS (accept snapshots via
  `cargo insta test -p katto-cli` then `cargo insta accept` if the insta CLI is available;
  otherwise move `.snap.new` → `.snap` manually after eyeballing, per the rules).

- [ ] **Step 5: Commit**

```bash
git add crates/katto-cli/ Cargo.lock
git commit -m "feat(cli): katto import/transcribe/plan/cut/auth surface with exit codes"
```

---

### Task 15: App pipeline job + bundle commands

**Files:**
- Modify: `src-tauri/src/keychain.rs` (add `read_key`)
- Modify: `src-tauri/src/commands/settings.rs` (add `planner_model`)
- Create: `src-tauri/src/commands/pipeline.rs`
- Modify: `src-tauri/src/commands.rs` (`pub mod pipeline;`)
- Modify: `src-tauri/src/lib.rs` (register 3 commands in `collect_commands![]`)
- Modify: `src-tauri/src/error.rs` (only if a new variant is needed — engine errors already
  funnel through `Error::Engine` via `From`)

**Invoke the `add-tauri-command` skill before writing the commands** — it owns the
checklist (registration, bindings regen, error mapping).

**Interfaces:**
- Consumes: engine pub API; jobs framework
  (`state.jobs.spawn(kind, label, payload_json, work)` → `jobs_repo::Job`;
  `ctx.progress(value, message)`; `JobContext { runtime, job_id, label }`); keychain
  (`SERVICE = "katto"`, `KeyService::{Elevenlabs, Anthropic}`); settings repo
  (`db::settings::{get, set}`); `db::events::record`; `broadcast::events_appended`.
  **[CONFIRM-AT-IMPL]**: quote-checked against the current tree, but `commands/ingest.rs`
  and friends are under concurrent revision — re-read `src-tauri/src/jobs.rs`,
  `src/keychain.rs`, `src/commands/settings.rs` before coding.
- Produces:

```rust
// keychain.rs — read side (pub(crate); value never crosses IPC)
/// Read a stored key; Ok(None) when absent. Never log the value.
pub fn read_key(service: KeyService) -> Result<Option<String>>;

// commands/settings.rs — Settings gains:
pub planner_model: String,        // default "claude-sonnet-4-6"; SettingsPatch gains Option<String>

// commands/pipeline.rs
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum PipelineEvent {
    Stage { name: StageName, progress: f64 },
    TranscriptReady { bundle_path: String },
    CutsPartial { cuts_so_far: Vec<katto_engine::schema::Cut> },
    Done { bundle_path: String },
    Failed { error: String },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum StageName { ExtractingAudio, Transcribing, DetectingCuts }

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct BundleSummary { pub path: String, pub name: String, pub has_transcript: bool, pub has_cuts: bool }

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct BundleData {
    pub root: String,
    pub source_path: String,
    pub frame_rate: katto_engine::Rational,
    pub duration_secs: f64,          // UI projection (to_secs_f64) — boundary only
    pub transcript: katto_engine::schema::Transcript,
    pub cuts: Option<katto_engine::schema::Cuts>,
    pub edits: Option<katto_engine::schema::Edits>,
}

#[tauri::command] #[specta::specta]
pub async fn plan_rough_cut(
    state: State<'_, AppState>,
    project_slug: String,
    footage_path: String,
    on_event: Channel<PipelineEvent>,
) -> Result<jobs_repo::Job>;

#[tauri::command] #[specta::specta]
pub async fn open_bundle(state: State<'_, AppState>, path: String) -> Result<BundleData>;

#[tauri::command] #[specta::specta]
pub async fn list_bundles(state: State<'_, AppState>, project_slug: String) -> Result<Vec<BundleSummary>>;
```

Notes: `katto_engine::schema::{Cut, Cuts, Transcript, Edits}` and `Rational` must derive
`specta::Type` to cross IPC — add `specta = { version = "=2.0.0-rc.25", features = ["derive"], optional = true }`
behind a `specta` cargo feature in the **engine** (engine stays UI-free: the feature is
off by default; src-tauri enables it: `katto-engine = { path = ..., features = ["specta"] }`;
derive gated with `#[cfg_attr(feature = "specta", derive(specta::Type))]`). This keeps the
one-way dependency rule intact (specta is a serialization-schema crate, not tauri).

`plan_rough_cut` behavior:
1. Resolve project + footage path (validate `footage_path` is inside the project's
   `footage/` dir — canonicalize + `starts_with`, mirroring the path-validation stance of
   the Phase-3 fix pass), resolve keys/settings **before** spawning: ElevenLabs key
   (`read_key(Elevenlabs)` → missing = typed error, no job), planner (settings
   `claude_path` or fresh detect → subprocess; else `read_key(Anthropic)` → http with
   settings `planner_model`; both missing → typed error naming the fix: the app surfaces
   "offer API-key mode" copy).
2. `state.jobs.spawn("cut_pipeline", &format!("Rough cut — {file_name}"), Some(payload_json), work)`
   where `payload_json` = `{"project_slug", "footage_path", "bundle_path"}` and the work
   closure owns the `Channel` (clone into an `Arc`).
3. Work sequence (each step also `ctx.progress()` so tray/dashboard mirror):
   `Stage{ExtractingAudio, 0.0}` → `engine::import::import(video, &project_audio_dir)` →
   `Stage{Transcribing, ~0.33}` heartbeat (a `tokio::time::interval` nudging progress
   toward 0.9 of the stage while awaiting) → `transcribe_into_bundle` →
   `TranscriptReady{bundle_path}` → `Stage{DetectingCuts, ~0.66}` → planner with a
   `PartialObserver` that sends `CutsPartial{cuts_so_far}` (subprocess mode) →
   `write_json_atomic(cuts.json)` → `db::events::record(conn, "rough_cut_planned",
   Some(slug), Some(&json!({"bundle": ..., "cuts": n, "flags": m}).to_string()))` +
   `broadcast::events_appended` → `Done{bundle_path}` → `Ok(())`.
   Any error → `on_event.send(Failed{error})` best-effort, then `Err(msg)` so the jobs
   framework does its fail + events-row + tray work (nothing fails silently, no double
   bookkeeping).
4. `open_bundle`: `spawn_blocking` → `engine::bundle::open` (typed `SourceMissing`
   surfaces via `Error::Engine`), map to `BundleData` (transcript required — a bundle
   without transcript.json returns `Error::Engine("bundle has no transcript yet")`).
5. `list_bundles`: scan `<project>/audio/*.kruproj` dirs (`spawn_blocking`), stat the
   artifact files for the two booleans.

- [ ] **Step 1: Write failing tests** (in `commands/pipeline.rs` `mod tests`, following the
  crate's existing in-module test style — pure helpers only; the command shells stay thin):

```rust
#[test]
fn footage_path_must_live_under_project_footage() {
    // pure helper: validate_footage_path(project_dir, footage_path) -> Result<PathBuf>
    let dir = tempfile::tempdir().unwrap();
    let footage = dir.path().join("footage");
    std::fs::create_dir_all(&footage).unwrap();
    let clip = footage.join("2026-07-22_001.mp4");
    std::fs::write(&clip, b"x").unwrap();
    assert!(validate_footage_path(dir.path(), &clip).is_ok());
    assert!(validate_footage_path(dir.path(), std::path::Path::new("/etc/passwd")).is_err());
}

#[test]
fn bundle_summary_from_dir_reads_artifact_presence() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("clip.kruproj");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("transcript.json"), b"{}").unwrap();
    let s = bundle_summary(&root).unwrap();
    assert!(s.has_transcript);
    assert!(!s.has_cuts);
}
```

  (src-tauri needs `tempfile` as dev-dep if not present — check; Phase 3's copy tests
  likely added it.)

- [ ] **Step 2: Run to verify failure** — `cargo test -p katto pipeline` (package name per
  src-tauri `Cargo.toml`; confirm with `cargo test -p katto --lib` conventions used by
  Phase 3) → compile errors.

- [ ] **Step 3: Implement** keychain `read_key`, settings `planner_model`
  (Settings/SettingsPatch/`read` fallback default `"claude-sonnet-4-6"`), the engine
  `specta` feature + cfg_attr derives, and `commands/pipeline.rs` per the behavior spec.
  Register `plan_rough_cut, open_bundle, list_bundles` in `collect_commands![]`
  (`src-tauri/src/lib.rs`).

- [ ] **Step 4: Regenerate bindings + run** — `just check` from the workspace root
  (runs the `export_bindings` test which rewrites `src/lib/ipc/bindings.gen.ts`; also
  fmt/clippy/tsc). Expected: green, bindings diff shows the three commands + new types.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/ crates/katto-engine/Cargo.toml crates/katto-engine/src/ \
        src/lib/ipc/bindings.gen.ts Cargo.lock
git commit -m "feat(app): plan_rough_cut pipeline job with streamed events, bundle commands"
```

---

### Task 16: Frontend — pipeline IPC, store, and step indicator

**Files:**
- Create: `src/lib/ipc/pipeline.ts`
- Create: `src/stores/pipeline.ts`
- Create: `src/features/pipeline/plan-steps.tsx`
- Create: `src/features/pipeline/plan-steps.test.tsx`
- Modify: the Phase-3 footage card component to add the "Plan rough cut" action —
  **[CONFIRM-AT-IMPL]** locate it first (`rg -l "FootageCard|footage" src/features/`);
  it was wired into `project-detail.tsx` in Phase 3. **The DateInput stash discipline
  applies to any commit touching `project-detail.tsx`** (stash the DateInput paths, commit,
  pop).

**Interfaces:**
- Consumes: generated bindings (`commands.planRoughCut`, `commands.openBundle`,
  `commands.listBundles`, types `PipelineEvent`, `StageName`, `BundleSummary`, `Job`) —
  confirm exact generated names in `bindings.gen.ts` after Task 15 (tauri-specta
  camelCases command names).
- Produces:

```ts
// src/lib/ipc/pipeline.ts — the only place invoke-side names appear
export function planRoughCut(projectSlug: string, footagePath: string,
  onEvent: (e: PipelineEvent) => void): Promise<Job>;
export function openBundle(path: string): Promise<BundleData>;
export function listBundles(projectSlug: string): Promise<BundleSummary[]>;

// src/stores/pipeline.ts — zustand, selector-syntax only (mirrors the ingest store's
// placement rationale: reachable from project detail + future surfaces without
// feature-to-feature imports)
export type StepState = "pending" | "active" | "done" | "failed";
export type PipelineRun = {
  jobId: string; projectSlug: string; footagePath: string; bundlePath: string | null;
  steps: Record<StageName, StepState>;
  stageProgress: number;                    // 0..1 within the active stage
  cutsSoFar: Cut[]; error: string | null; finished: boolean;
};
export const usePipelineStore: /* zustand store */;
// actions: start(slug, path) -> wires planRoughCut + event reduction; reset(slug|jobId)
// pure reducer exported for tests:
export function reduceEvent(run: PipelineRun, e: PipelineEvent): PipelineRun;
```

Step indicator design (Dribbble-grounded, design-system-true — dense workshop, zero
decoration):

- A vertical three-row list inside the footage card's expanding footer (not a modal):
  rows are fixed 30px; each row = state dot (8px, `--queued` pending / ember pulsing
  active / `--done` complete / `--failed`) + step label in sans ("Extracting audio",
  "Transcribing", "Detecting cuts") + right-aligned **mono, tabular-nums** detail
  (elapsed `m:ss` for the active step; `—` pending; blank done). A 1px `--hairline`
  connector joins the dots vertically (taken from the reviewed step-timeline shots;
  rejected: percent rings, skeleton shimmer, gradients, "AI is thinking…" copy).
- Active step also renders the 4px ember progress track (existing progress primitive)
  under the row when `stageProgress > 0`.
- During DetectingCuts, the detail column shows a live count: `12 cuts` — ticking up as
  `CutsPartial` events land (count only — **no scores, no confidence numbers**).
- Failure state: the failed row's dot goes `--failed`, and one line of plain-language
  error copy below ("claude exited with an error — open Settings to add an Anthropic API
  key instead." for subprocess failures; error text from the event otherwise). No toast
  spam — the jobs dashboard already mirrors it.
- Reduced motion: the pulsing active dot is static under `prefers-reduced-motion`.

- [ ] **Step 1: Write failing reducer tests** (`src/stores/pipeline.test.ts`):

```ts
import { describe, expect, it } from "vitest";
import { initialRun, reduceEvent } from "./pipeline";

describe("pipeline event reduction", () => {
  it("marks earlier stages done when a later stage starts", () => {
    let run = initialRun("slug", "/f/clip.mp4");
    run = reduceEvent(run, { type: "stage", name: "transcribing", progress: 0 });
    expect(run.steps.extracting_audio).toBe("done");
    expect(run.steps.transcribing).toBe("active");
  });

  it("stores bundle path on transcript_ready", () => {
    let run = initialRun("slug", "/f/clip.mp4");
    run = reduceEvent(run, { type: "transcript_ready", bundle_path: "/p/audio/clip.kruproj" });
    expect(run.bundlePath).toBe("/p/audio/clip.kruproj");
  });

  it("accumulates cuts_partial and finishes on done", () => {
    let run = initialRun("slug", "/f/clip.mp4");
    run = reduceEvent(run, { type: "cuts_partial", cuts_so_far: [{ start: 1, end: 2, reason: "filler", excerpt: "um" }] });
    expect(run.cutsSoFar).toHaveLength(1);
    run = reduceEvent(run, { type: "done", bundle_path: "/b" });
    expect(run.finished).toBe(true);
    expect(run.steps.detecting_cuts).toBe("done");
  });

  it("failed marks the active step failed and records the error", () => {
    let run = initialRun("slug", "/f/clip.mp4");
    run = reduceEvent(run, { type: "stage", name: "transcribing", progress: 0.1 });
    run = reduceEvent(run, { type: "failed", error: "elevenlabs auth: bad key" });
    expect(run.steps.transcribing).toBe("failed");
    expect(run.error).toContain("elevenlabs");
  });
});
```

(Exact wire tags — `"stage"` vs `"Stage"`, field casing — must match the generated
`PipelineEvent` type in `bindings.gen.ts`; write the tests against the real generated
type so tsc enforces it.)

Component test (`plan-steps.test.tsx`): render `PlanSteps` with a store seeded mid-run;
assert by role/text: three step labels visible, active step's row exposes the elapsed
detail, failed run shows the error copy. No class/DOM-snapshot assertions.

- [ ] **Step 2: Run to verify failure** — `bunx vitest run src/stores/pipeline.test.ts` → fails (module missing).

- [ ] **Step 3: Implement** store + component + card wiring. The footage-card action
  appears per footage file row ("Plan rough cut", ghost button, `cursor-default`); when a
  run exists for that file it swaps to the step list. Disable the action while a run is
  active for the same file. After `done`, show a "Review cut plan" secondary button
  (navigates to the Task-18 editor route with the bundle path).

- [ ] **Step 4: Run** — `bunx vitest run src/stores src/features/pipeline` → PASS; `just check` → green.

- [ ] **Step 5: Commit** (stash discipline if `project-detail.tsx` is touched):

```bash
git stash push src/components/ui/date-input.tsx src/components/ui/date-input.test.tsx \
  src/features/projects/detail/project-detail.test.tsx src/styles/main.css 2>/dev/null || true
# stage ONLY the pipeline work + the intended project-detail hunks:
git add src/lib/ipc/pipeline.ts src/stores/pipeline.ts src/stores/pipeline.test.ts src/features/pipeline/
git add -p src/features/projects/detail/project-detail.tsx   # pick pipeline hunks only if file was stashed-around
git commit -m "feat(pipeline): footage-card rough-cut runner with streamed step indicator"
git stash pop 2>/dev/null || true
```

(If `git stash push` on those mixed files proves too fiddly because Phase-4 and DateInput
hunks interleave in `project-detail.tsx`, use `git add -p` alone and verify with
`git diff --cached` that no DateInput hunk is staged — that is the actual requirement.)

---

### Task 17: Editor model — token spans + overlay classification (pure)

**Files:**
- Create: `src/features/editor/model/tokens.ts`, `tokens.test.ts`
- Create: `src/features/editor/model/overlay.ts`, `overlay.test.ts`

**Interfaces:**
- Consumes: generated types `Transcript`/`WordEntry`/`Cuts` from `bindings.gen.ts`
  (post-Task-15).
- Produces:

```ts
// tokens.ts — pure, no React
export type TokenSpan = {
  index: number;                 // position in transcript.words
  text: string;
  kind: "word" | "spacing" | "audio_event";
  start: number; end: number;    // seconds (UI boundary floats)
  speakerId: string | null;
};
export type Paragraph = { tokens: TokenSpan[] };

export function buildTokenSpans(words: WordEntry[]): TokenSpan[];
/** Group spans into paragraphs: break on speaker change or an inter-token gap > 1.5s. */
export function groupParagraphs(spans: TokenSpan[]): Paragraph[];
/** Binary search: the token whose [start,end) contains t, else the nearest following token. */
export function tokenAtTime(spans: TokenSpan[], t: number): TokenSpan | null;

// overlay.ts — pure, no React
export type OverlayKind = "cut" | "discretionary" | "flag";
export type TokenOverlay = { kind: OverlayKind; entryIndex: number } | null;
/** Per-token classification; precedence cut > discretionary > flag. A token is covered
 *  when its midpoint lies inside the span (boundaries sit on token edges by invariant 7,
 *  so midpoint containment is exact). */
export function classifyTokens(spans: TokenSpan[], cuts: Cuts): TokenOverlay[];
/** Flags are seek targets: entryIndex -> flag start seconds. */
export function flagSeekTimes(cuts: Cuts): number[];
```

- [ ] **Step 1: Write failing tests** (representative — write the full set):

```ts
// tokens.test.ts
const words: WordEntry[] = [
  { type: "word", text: "So", start: 0.12, end: 0.34, logprob: -0.2, speaker_id: "speaker_0" },
  { type: "spacing", text: " ", start: 0.34, end: 0.47 },
  { type: "word", text: "today", start: 0.47, end: 0.9, logprob: -0.1, speaker_id: "speaker_0" },
  // 2s gap -> paragraph break
  { type: "word", text: "Next", start: 2.9, end: 3.2, logprob: -0.1, speaker_id: "speaker_0" },
];

it("builds spans preserving indices", () => {
  const spans = buildTokenSpans(words);
  expect(spans[2]).toMatchObject({ index: 2, text: "today", kind: "word" });
});

it("breaks paragraphs on >1.5s gaps", () => {
  const paras = groupParagraphs(buildTokenSpans(words));
  expect(paras).toHaveLength(2);
  expect(paras[1].tokens[0].text).toBe("Next");
});

it("tokenAtTime finds containing token and clamps to next", () => {
  const spans = buildTokenSpans(words);
  expect(tokenAtTime(spans, 0.5)?.text).toBe("today");
  expect(tokenAtTime(spans, 1.5)?.text).toBe("Next"); // in the gap -> next token
  expect(tokenAtTime(spans, 99)).toBeNull();
});

// overlay.test.ts
const cuts: Cuts = {
  source_duration_secs: 10,
  cuts: [{ start: 0.12, end: 0.47, reason: "filler", excerpt: "So " }],
  discretionary: [{ start: 0.47, end: 0.9, reason: "other", excerpt: "today", note: "n", confidence: "medium" }],
  flags: [{ start: 2.9, end: 3.2, reason: "low_confidence", excerpt: "Next", logprob: -7.8 }],
  total_cut_secs: 0.35,
};

it("classifies each covered token once with precedence", () => {
  const overlays = classifyTokens(buildTokenSpans(words), cuts);
  expect(overlays[0]).toMatchObject({ kind: "cut", entryIndex: 0 });      // "So"
  expect(overlays[1]).toMatchObject({ kind: "cut", entryIndex: 0 });      // spacing inside cut
  expect(overlays[2]).toMatchObject({ kind: "discretionary", entryIndex: 0 });
  expect(overlays[3]).toMatchObject({ kind: "flag", entryIndex: 0 });
});

it("uncovered tokens are null", () => {
  const overlays = classifyTokens(buildTokenSpans(words), { ...cuts, cuts: [], discretionary: [], flags: [] });
  expect(overlays.every((o) => o === null)).toBe(true);
});
```

- [ ] **Step 2: Run to verify failure** — `bunx vitest run src/features/editor/model` → fails.

- [ ] **Step 3: Implement** both modules (no React imports; `noUncheckedIndexedAccess`-clean).

- [ ] **Step 4: Run** — `bunx vitest run src/features/editor/model` → PASS.

- [ ] **Step 5: Commit**

```bash
git add src/features/editor/model/
git commit -m "feat(editor): pure token-span and overlay classification models"
```

---

### Task 18: Editor surface — read-only review

**Files:**
- Create: `src/features/editor/transcript-pane.tsx`, `transcript-pane.test.tsx`
- Create: `src/features/editor/video-pane.tsx`
- Create: `src/features/editor/editor-view.tsx`
- Modify: router/composition in `src/app/` (new route) — **invoke the
  `add-feature-surface` skill first**; it owns the route/sidebar checklist.
- Modify: project detail — bundles list ("Cut plans" section under the footage card:
  `listBundles` query; row = bundle name + `● transcript` / `● cuts` presence chips +
  click → editor route). Stash discipline again if `project-detail.tsx` is touched.

**Interfaces:**
- Consumes: `openBundle` IPC, Task-17 models, `convertFileSrc` from
  `@tauri-apps/api/core`.
- Produces:

```ts
// transcript-pane.tsx
export function TranscriptPane(props: {
  transcript: Transcript;
  cuts: Cuts | null;
  onSeek: (seconds: number) => void;
}): JSX.Element;

// video-pane.tsx
export type VideoPaneHandle = { seek: (seconds: number) => void };
export const VideoPane: ForwardRefExoticComponent<{ sourcePath: string } & RefAttributes<VideoPaneHandle>>;
// <video src={convertFileSrc(sourcePath)} controls> — media via asset protocol ONLY

// editor-view.tsx — route target `/projects/:slug/cut?bundle=<path>` (exact route shape
// per the add-feature-surface skill's conventions): TanStack query on openBundle,
// two-pane layout inside [data-scroll-root] (video pane pinned, transcript scrolls)
```

Design (from the Sonet/Descript-style reference, filtered hard through the design system):
- Transcript is the primary surface: comfortable measure (~65ch), body in sans
  (`--sans`), 15px/1.7; paragraphs from `groupParagraphs`; a muted gutter label at each
  paragraph start with the **mono tabular** start timecode (`m:ss.s`) — mono is machine
  data here, which is its sanctioned use. No speaker chrome beyond a `--fg-muted` name
  when `speaker_id` changes (single-speaker footage usually renders none).
- Overlay rendering (state shown once, no rails): `cut` → `--fg-faint` +
  `line-through`; `discretionary` → dotted underline in `--warn` (amber) +
  `title={note}`; `flag` → `--warn`-tinted background highlight (opt out of grain per the
  translucent-fill rule: `style={{ backgroundImage: "none" }}` if placed on a Card
  surface). Audio-event tokens (`[breath]`…) render in `--fg-muted` italics.
- Interaction: click any token → `onSeek(token.start)`; click a flag highlight → seek
  only (PRD: flags are review targets, never removals). `cursor: default` everywhere
  (native-app signal); no hover hand.
- Video pane: plain `<video controls>` on `--bg`, no custom chrome this phase.
- Empty/loading: "Opening bundle…" single line; a bundle whose source is missing surfaces
  the typed error copy: "Source video not found at <path>. Relocation arrives with the
  editor phase." (error text from `{kind: "engine", message}`).

- [ ] **Step 1: Write failing component test** (`transcript-pane.test.tsx`):

```ts
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

it("clicking a word seeks to its start", () => {
  const onSeek = vi.fn();
  render(<TranscriptPane transcript={fixtureTranscript} cuts={null} onSeek={onSeek} />);
  fireEvent.click(screen.getByText("today"));
  expect(onSeek).toHaveBeenCalledWith(0.47);
});

it("cut tokens are marked struck-through for assistive tech", () => {
  render(<TranscriptPane transcript={fixtureTranscript} cuts={fixtureCuts} onSeek={() => {}} />);
  // behavior, not classes: the cut word is inside a <del> element
  expect(screen.getByText("So").closest("del")).not.toBeNull();
});

it("clicking a flagged word seeks and does not strike it", () => {
  const onSeek = vi.fn();
  render(<TranscriptPane transcript={fixtureTranscript} cuts={fixtureCuts} onSeek={onSeek} />);
  const flagged = screen.getByText("Next");
  expect(flagged.closest("del")).toBeNull();
  fireEvent.click(flagged);
  expect(onSeek).toHaveBeenCalledWith(2.9);
});
```

(Use `<del>` for cut spans and `<mark>` for flags — semantic elements make the tests
behavioral instead of class-based, per the frontend testing rules. Fixtures shared from
the Task-17 test data — hoist into `src/features/editor/model/fixtures.ts` (test-only
import) or `src/test/fixtures/`.)

- [ ] **Step 2: Run to verify failure** — `bunx vitest run src/features/editor` → fails.

- [ ] **Step 3: Implement** panes + view + route (skill first) + bundles list on project
  detail. `editor-view` holds the `VideoPaneHandle` ref and passes `onSeek` down.

- [ ] **Step 4: Run** — `bunx vitest run src/features/editor` → PASS; `just check` → green.

- [ ] **Step 5: Commit** (stash discipline for `project-detail.tsx` as in Task 16):

```bash
git add src/features/editor/ src/app/
git add -p src/features/projects/detail/project-detail.tsx
git commit -m "feat(editor): read-only cut review - transcript pane, video pane, bundle route"
```

---

### Task 19: Phase close-out — docs, owner checklist, gate

**Files:**
- Modify: `docs/overnight-run.md` (Phase 4 section)
- Modify: `prd/index.md` (phase 4 status → implemented, pending verification)

- [ ] **Step 1: Run the reviewers.** Per CLAUDE.md: run `rust-reviewer` on the
  crates/src-tauri diff and `frontend-reviewer` on the `src/` diff; fix confirmed
  findings (the overnight workflow's adversarial review agent also runs — coordinate via
  the team lead).

- [ ] **Step 2: Full gate.** Run `just check` from the workspace root. Paste the tail of
  its output into the task report. Must be green.

- [ ] **Step 3: Update `docs/overnight-run.md`** — replace the Phase 4 placeholder with:

```markdown
## Phase 4 — Cut Pipeline

_Status: implemented, pending review_

### Owner visual/manual checks (tick after testing)

- [ ] With the dev app running and a project containing real footage: the footage card
      shows "Plan rough cut" per clip; clicking it starts the three-step indicator
      (Extracting audio → Transcribing → Detecting cuts) with a live elapsed timer and
      the tray/dashboard mirroring the `cut_pipeline` job.
- [ ] Transcript appears in the review surface as soon as transcription lands (before
      planning finishes); cut count ticks up during Detecting cuts (subprocess mode).
- [ ] REAL-KEY CHECK: ElevenLabs key from onboarding is used (no key → typed error with
      fix copy, no job started). Requires the real ElevenLabs key in the keychain.
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
- [ ] `katto cut <video>` (CLI) produces the same bundle headless; `katto auth status`
      and `--json` output look right; exit codes: usage error 2, missing file 1.
- [ ] `events` log shows `rough_cut_planned {bundle, cuts, flags}` after a successful run.
- [ ] Failure paths: yank network mid-transcription → job fails with typed ElevenLabs
      error, no partial transcript.json in the bundle; feed a deliberately huge clip and
      cancel is NOT available (by design — jobs run to completion; confirm this is
      acceptable or file for Phase 5).
- [ ] Ignored tests to run by hand when convenient:
      `KATTO_TEST_CLIP=/path/to/clip.mp4 cargo test -p katto-engine import_real_clip_end_to_end -- --ignored`
      and the `full_pipeline_real_binaries` test (needs ELEVENLABS_API_KEY exported).

Automated coverage: Rational property tests, probe-timing fixtures (incl. 29.97/59.94),
schema round-trips, 6-fixture validator table, exhaustive merge tests, bundle round-trip,
transcribe/HTTP-planner wiremock suites, stream-accum + partial-extractor fixtures,
retry-loop stub tests, mocked pipeline integration, CLI snapshots + exit codes, pipeline
reducer + editor model/component tests. `just check` green at phase end.

Design deviations from the plan (for review): <fill during implementation — list every
divergence the implementing agent made, or "none">.
```

- [ ] **Step 4: Update `prd/index.md`** phase-4 status cell to match how Phase 3 was
  marked ("implemented, pending verification") — same wording convention.

- [ ] **Step 5: Commit**

```bash
git add docs/overnight-run.md prd/index.md
git commit -m "docs(prd): mark phase 4 cut pipeline implemented pending verification"
```

---

## Self-Review (performed while writing — verified against `prd/phase-4.md`)

Coverage map, PRD scope row → task: Rational complete → 1 · schema types real → 3/5/6 ·
cuts.json validation → 4 · float↔Rational boundary → 5 (`CutPlan::from_wire`, sole
conversion site) + 2/7 (Rational duration; `duration_s` untouched) · import → 7 ·
transcribe → 8 · CutPlanner trait → 9 · SubprocessClaudePlanner → 10 ·
HttpAnthropicPlanner → 11 · validate-retry loop → 12 · cuts↔edits merge → 5 · bundle
round-trip → 6 · CLI → 14 · pipeline UI (one job, Channel, transcript-before-planning,
incremental cuts) → 15/16 · read-only review (token spans, gray strikethrough, amber
dotted, flag highlight seek-only, convertFileSrc) → 17/18 · error handling table → 8
(ElevenLabs typed/retryable/no partial writes), 10/15 (claude non-zero stderr + API-key
offer), 7 (ffmpeg stderr, bundle intact), 6 (SourceMissing typed), 12 (retry-once then
raw), jobs framework (events, no silent failure) · testing section → each task's tests +
13 (integration, `#[ignore]` real-network) + 19 (manual checkpoint) · data-model deltas:
none (one settings **key** added — `planner_model` — a k/v row, not a schema change; no
migration) · out-of-scope respected (no editing, no exports, no keep-window port, no
dock).

