# Phase 3 — SD Ingest Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. TDD per `.claude/rules/testing.md` — the failing test is written and *run red* before the implementation in every logic task.

**Goal:** Insert a camera card → notification → answer one question (which project?) → verified, renamed footage in the project's flat `footage/` folder → card ejected. Dumb and deterministic; **no AI in this path** (D10).

**Architecture:** Pure media logic (card recognition, clip enumeration/grouping, rename/sequence math, size verification, ffprobe JSON parsing) lives in `crates/katto-engine` as pure functions over in-memory representations. The filesystem-, spawn-, watcher-, and IPC-touching shells (volume watcher, ffprobe spawn, copy job, Tauri commands) live in `src-tauri` and call the engine. The copy job plugs into the Phase-1 jobs framework (`JobRuntime::spawn` → `Channel<JobProgress>` → tray mirror → terminal `events` row). The React `src/features/ingest/` surface reuses the existing jobs progress component and the dialog/sheet primitives.

**Tech Stack:** Rust (engine + Tauri 2 app crate), React 19 + TypeScript + Vite + Tailwind v4, `notify` 8.2 (volume watch), `fs4` (free space), `ffprobe` 8.x (spawned), `diskutil eject` (spawned), tauri-specta typed IPC, TanStack Query + Zustand.

## Global Constraints

- **Workspace layout (CLAUDE.md):** `crates/katto-engine` is pure — it never depends on `tauri`, `specta`, or any UI concern. `src-tauri` → engine is the only allowed direction. Pure ingest logic (recognize, enumerate, naming, verify, ffprobe parser) goes in the engine; watcher/spawn/copy-job/commands go in `src-tauri`.
- **Rust rules (`.claude/rules/rust.md`):** RFC-430 naming; 2018 module style (parent `foo.rs` + children in `foo/`, **never a new `mod.rs`**); one `thiserror` enum per crate in `error.rs`; **no `unwrap()`/`expect()` outside `#[cfg(test)]`** — propagate with `?`; engine `#![warn(missing_docs)]` (every `pub` item gets a `///`, `Result`-returning `pub` fns get an `# Errors` section); test names are `<scenario>_<expected>` with no `test_` prefix; fixtures under `tests/fixtures/` loaded relative to `CARGO_MANIFEST_DIR`.
- **Tauri rules (`.claude/rules/tauri-commands.md`):** commands are thin shells (unwrap args → call engine/db/jobs → map errors); **no SQL in commands** — `db/` repos own queries; fallible commands return `Result<T, Error>` (the app-wide tagged enum); async commands take owned params; CPU/blocking work in `spawn_blocking`; command-scoped streaming uses `tauri::ipc::Channel<T>`, app-wide state uses broadcast events; media bytes never cross `invoke`.
- **Testing rules (`.claude/rules/testing.md`):** TDD default for logic; DB tests use `test_db()` (in-memory, all migrations, no WAL); deterministic subprocess layers (ffprobe argv, ffprobe JSON parse) are pure functions tested without spawning; the single spawn call sites stay thin, covered by `#[ignore]`d integration tests + manual hardware checkpoints; fixtures are real files/trees under `tests/fixtures/`.
- **Design-system rules (`.claude/rules/design-system.md`):** tokens never literals (`--surface --border --fg --fg-muted --ember --on-ember` + semantic `--done --failed --queued --warn`); serif display / SF Pro UI / **SF Mono for data only** (paths, sizes, counts, timecodes with `tabular-nums`); Phosphor icons Regular; `--r` controls / `--r-lg` cards; `cursor: default` on controls; **banned AI tells** — mono eyebrows, accent side-rails, cloud-upload illustrations as hero, gradient heroes, everything-centered, everything-`rounded-2xl`; copy from the user's side ("Import 14 clips", not "Effortlessly transfer your media").
- **Frontend rules (`.claude/rules/frontend.md`):** feature folders `src/features/ingest/{components,hooks,store,model}` (create only what's used); one-way imports `shared → features → app`; no barrels; IPC only through `src/lib/ipc/ingest.ts` typed wrappers over generated bindings (never `invoke("string")` in feature code, never hand-edit `bindings.gen.ts`); `strict` + `noUncheckedIndexedAccess` + `verbatimModuleSyntax`, no `any`; Vitest tests target `model/` pure fns and user-visible behavior, never Tailwind classes / DOM snapshots; mock IPC with `mockIPC`, `clearMocks()` in `afterEach`.
- **Invariants (CLAUDE.md):** the card is **never written to** (source opened read-only); **copy-only** (never move/delete source); nothing fails silently (every op is a `jobs` row + terminal `events` row); no numeric scoring; folders are truth; versioned artifacts never overwritten. Rational time in the engine; `f64` only at UI/model boundaries.
- **Commits:** Conventional Commits, one concern per commit, tests travel with their feature commit, via the `ship` skill. Gate every phase-done claim on **`just check`** green (fmt-check + clippy `-D warnings` + cargo test + tsc) run from the workspace root; paste the tail when reporting.
- **NEVER stage the uncommitted DateInput work.** `git add` only the explicit paths a task lists — **never `git add -A`/`git add .`**. Off-limits paths: `src/components/ui/date-input.tsx`, `src/components/ui/date-input.test.tsx`, and the working hunks in `src/features/projects/detail/project-detail.tsx`, `src/features/projects/detail/project-detail.test.tsx`, `src/styles/main.css`. **Task 15 edits `project-detail.tsx`, which already has uncommitted DateInput hunks** — before committing Task 15, `git stash push -- src/features/projects/detail/project-detail.tsx src/features/projects/detail/project-detail.test.tsx src/styles/main.css`, commit only the ingest additions, then `git stash pop`. (See Task 15 for the exact sequence.)

---

## Design notes (Dribbble grounding + katto design system)

Design grounding was done via Dribbble (owner signed in): searches "file import modal" and "file transfer progress", ~6 shots reviewed. katto's committed design system is the governing brief; Dribbble supplied concrete layout ideas, filtered against the banned-AI-tells list.

**Ideas taken:**
- **The header/CTA names the destination.** (Robert Kreft "Upload files to project **Alpha**" / "Import to Alpha".) The import sheet title reads `Import 14 clips` and the primary button reads `Import to <project>` — this *is* katto's one-question model made visible.
- **Per-file row anatomy that transitions in place.** (Nabil Fikri, Robert Kreft.) Each clip row: Phosphor file-type glyph · destination name (mono) · size (mono, right-aligned) · a status affordance that moves `queued → copying (inline 4px ember bar + %) → verified (● done check) → failed (● failed)`. The same row is reused verbatim by the progress panel.
- **Grouped list with a per-group select-all.** (Robert Kreft's file list; katto groups by card substructure — `CLIP`, `SUB`, or the `DCIM/100APPLE` subfolder — with a group header + select-all checkbox.)
- **macOS Finder copy-sheet phrasing for progress.** (Thomas René "Copying 21 items from 'Downloads' to 'Videos'".) The progress panel reads `Copying 14 clips → <project>` with one overall 4px ember bar and a `9 of 14 · 6.2 GB of 22 GB` counter (mono, `tabular-nums`), per-file rows beneath. On-brand for a macOS-native app.

**Ideas rejected as off-brand:**
- The dashed **"drag & drop / click to upload" hero drop-zone** as the primary affordance — the SD card is *auto-detected and pre-selected*; there is no browse step. A drop target belongs only to the manual iPhone path on the project-detail footage card, never the ingest sheet.
- Radial progress **rings**, throughput (MB/s), and ETA countdown chrome — katto's system specifies a 4px linear track, ember fill, calm density. Count + bytes is enough.
- Purple/green accents, `rounded-2xl` everything, centered layouts, cloud-upload illustrations, promotional subtitles — replaced with the ember accent, `--r`/`--r-lg`, left-aligned dense rows, Phosphor `SdCard`/`FilmSlate` glyphs, and copy written from the user's side.

**Tokens used:** surfaces `--surface`/`--surface-2` + `--border`; text `--fg`/`--fg-muted`/`--fg-faint`; accent `--ember`/`--on-ember`; state `--done`/`--failed`/`--queued`/`--warn`; sizes/counts/paths in `--mono` with `tabular-nums`; card radius `--r-lg`, control radius `--r`. Free-space warning uses `--warn` and opts out of grain (`style={{ backgroundImage: "none" }}` per the translucent-fill rule).

---

## File structure

**Engine (`crates/katto-engine/`) — pure, tested without a filesystem or subprocess:**
- Create `src/error.rs` — `Error` (thiserror) + `Result<T>`; the crate's single error enum.
- Create `src/ffprobe.rs` — `parse_probe(json: &str) -> Result<MediaInfo>` + `MediaInfo`. Pure JSON→struct, shared with Phase 4.
- Create `src/ingest.rs` — module parent; shared ingest types (`CardKind`, `VolumeTree`, `Card`, `FileEntry`, `ClipEntry`, `ClipGroup`, `Rename`, `VerifyError`).
- Create `src/ingest/recognize.rs` — `recognize(tree: &VolumeTree) -> Option<Card>`.
- Create `src/ingest/enumerate.rs` — `enumerate(kind: CardKind, files: &[FileEntry]) -> Vec<ClipGroup>`.
- Create `src/ingest/naming.rs` — `next_sequence`, `dest_filename`, `plan_renames`.
- Create `src/ingest/verify.rs` — `verify(expected, actual) -> Vec<VerifyError>`.
- Modify `src/lib.rs` — add `#![warn(missing_docs)]`, `pub mod error; pub mod ffprobe; pub mod ingest;`, re-exports.
- Create `tests/fixtures/cards/` — real directory trees: `sony/`, `generic-dcim/`, `iphone-dcim/`, `not-a-card/`.
- Create `tests/fixtures/ffprobe/` — captured ffprobe JSON: `xavc-hs-4k60.json`, `iphone-hevc.json`, `df-2997.json`.
- Create `tests/recognize.rs`, `tests/enumerate.rs` — integration tests walking the real fixture trees into `VolumeTree`/`FileEntry` and asserting engine output.

**App crate (`src-tauri/`):**
- Modify `Cargo.toml` — add `notify = "8.2"`.
- Modify `src/error.rs` — add `Engine(String)` variant + `From<katto_engine::Error>`.
- Create `src/ffprobe.rs` — `probe_clip(path: &Path) -> Result<MediaInfo>`; the single ffprobe spawn site → `katto_engine::ffprobe::parse_probe`.
- Create `src/volumes.rs` — the non-recursive `/Volumes` watcher; `start_watcher(app, ingest, studio_root)`; readability probe + debounce; walk → `recognize` → `enumerate` → build `CardOffer` → broadcast + notify.
- Create `src/ingest.rs` + `src/ingest/copy.rs` — the copy job (`run_copy_job`): copy source → `<dest>.partial` → verify size → rename `.partial` → `<dest>`; quarantine on failure; `ingested` events row on success.
- Create `src/commands/ingest.rs` — `card_offer`, `start_ingest`, `eject_card`, `import_files`.
- Modify `src/commands.rs` — add `pub mod ingest;`.
- Modify `src/state.rs` — add `IngestState { current: Mutex<Option<CardOffer>> }` (managed separately).
- Modify `src/broadcast.rs` — add `CardDetected { offer }` + `CardRemoved` events + emit helpers.
- Modify `src/notify.rs` — add `Route::Ingest` (`as_wire` → `"ingest"`, parse `"ingest"`).
- Modify `src/lib.rs` — register the 4 commands + 2 events in `specta_builder()`, `.manage(IngestState::default())`, start the watcher in `setup`, regenerate `bindings.gen.ts`.

**Frontend (`src/`):**
- Create `src/lib/ipc/ingest.ts` — typed wrappers + `ingestKeys`.
- Create `src/components/ui/checkbox.tsx`, `src/components/ui/callout.tsx` (via shadcn CLI where available, else the code given).
- Create `src/features/ingest/model/select.ts` + `.test.ts` — pure selection/size/default-project/free-space math.
- Create `src/features/ingest/hooks/use-card-offer.ts` — TanStack Query + `card-detected`/`card-removed` event subscription.
- Create `src/features/ingest/store/ingest-sheet.ts` — Zustand sheet open/target state.
- Create `src/features/ingest/components/import-sheet.tsx`, `clip-group-list.tsx`, `ingest-progress.tsx`.
- Modify `src/features/projects/detail/project-detail.tsx` — add a footage card with the `onDragDropEvent` manual-import path (Task 15; stash-gated commit).
- Modify `src/app/` composition + `src/hooks/use-deep-link-router.ts` — mount the import sheet globally, route `katto://ingest`.

---

## Task 1: Engine error enum + app-crate wrapping

**Files:**
- Create: `crates/katto-engine/src/error.rs`
- Modify: `crates/katto-engine/src/lib.rs`
- Modify: `src-tauri/src/error.rs`

**Interfaces:**
- Produces: `katto_engine::Error` (thiserror enum), `katto_engine::Result<T>`; app `Error::Engine(String)` with `From<katto_engine::Error>`.

- [ ] **Step 1: Write the failing test** — add to `crates/katto-engine/src/error.rs`:

```rust
//! The engine's single error type.

/// Errors returned by the pure media pipeline (ffprobe parsing, ingest logic).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// ffprobe output could not be parsed into [`crate::ffprobe::MediaInfo`].
    #[error("ffprobe: {0}")]
    Probe(String),
}

/// Convenience alias for engine results.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_error_displays_its_message() {
        let err = Error::Probe("no video stream".to_string());
        assert_eq!(err.to_string(), "ffprobe: no video stream");
    }
}
```

- [ ] **Step 2: Wire the module** — in `crates/katto-engine/src/lib.rs`, add at the very top `#![warn(missing_docs)]` and add `pub mod error;` with `pub use error::{Error, Result};` (remove the placeholder `add()` fn if it is unused elsewhere — grep first: `grep -rn "katto_engine::add\|engine::add" crates src-tauri`).

- [ ] **Step 3: Run the test red→green** — Run: `cargo test -p katto-engine error::tests::probe_error_displays_its_message`. Expected: compiles and PASSES.

- [ ] **Step 4: Wrap in the app error** — in `src-tauri/src/error.rs`, add a variant after `ShortcutUnavailable`:

```rust
    #[error("{0}")]
    Engine(String),
```

and add, next to the other `From` impls:

```rust
impl From<katto_engine::Error> for Error {
    fn from(err: katto_engine::Error) -> Self {
        Error::Engine(err.to_string())
    }
}
```

(The file's established convention is single-`String` variants + hand-written `From` mapping to `.to_string()`, preserving the tagged `{ kind, message }` wire shape — this overrides the generic "wrap via `#[from]`" rule, which would nest a non-`Serialize` payload.)

- [ ] **Step 5: Verify the crate builds** — Run: `cargo build -p katto-engine -p katto` . Expected: builds clean.

- [ ] **Step 6: Commit**

```bash
git add crates/katto-engine/src/error.rs crates/katto-engine/src/lib.rs src-tauri/src/error.rs
git commit -m "feat(engine): add error enum and app-crate wrapping"
```

---

## Task 2: ffprobe pure JSON parser (`MediaInfo`)

Pure `serde_json` parse of `ffprobe -show_streams -show_format -print_format json` output. Verified field shapes (ffprobe 8.1.2): `streams[].codec_name` (string), `streams[].codec_type` (`"video"`/`"audio"`), `streams[].width`/`height` (int), `streams[].r_frame_rate` (`"30000/1001"`), `streams[].duration` (string), `format.duration` (string, seconds), `format.size` (string, bytes). Numbers arrive as JSON strings.

**Files:**
- Create: `crates/katto-engine/src/ffprobe.rs`
- Create: `crates/katto-engine/tests/fixtures/ffprobe/{xavc-hs-4k60.json,iphone-hevc.json,df-2997.json}`
- Modify: `crates/katto-engine/src/lib.rs`

**Interfaces:**
- Consumes: `katto_engine::Result`, `katto_engine::Rational { num: i64, den: u32 }`.
- Produces: `MediaInfo { codec_name: Option<String>, width: Option<u32>, height: Option<u32>, duration_s: Option<f64>, fps: Option<Rational> }`; `parse_probe(json: &str) -> Result<MediaInfo>`.

- [ ] **Step 1: Capture the fixtures** — generate a real sample and hand-write the two others by copying its shape with edited values. Run:

```bash
cd crates/katto-engine && mkdir -p tests/fixtures/ffprobe
ffmpeg -f lavfi -i testsrc=duration=2:size=3840x2160:rate=60000/1001 -c:v libx264 -y /tmp/s.mp4 >/dev/null 2>&1
ffprobe -v quiet -print_format json -show_streams -show_format /tmp/s.mp4 > tests/fixtures/ffprobe/xavc-hs-4k60.json
```

Then create `iphone-hevc.json` (copy the file, set the video stream's `"codec_name": "hevc"`, `"width": 1920`, `"height": 1080`, `"r_frame_rate": "30/1"`, `format.duration` `"5.100000"`) and `df-2997.json` (set `"r_frame_rate": "30000/1001"`, `format.duration` `"12.345000"`). These are real JSON documents, not stubs.

- [ ] **Step 2: Write the failing test** — in `crates/katto-engine/src/ffprobe.rs`:

```rust
//! Pure parser for `ffprobe -show_streams -show_format -print_format json`
//! output. The single spawn site lives in the app crate; this module only
//! turns captured JSON into a [`MediaInfo`], so it is unit-tested without
//! running ffprobe.

use serde_json::Value;

use crate::error::{Error, Result};
use crate::rational::Rational;

/// The subset of ffprobe metadata katto needs: the first video stream's codec
/// and dimensions, container duration in seconds, and frame rate as an exact
/// [`Rational`] (so drop-frame rates like `30000/1001` survive). Every field is
/// optional — enumeration never blocks on metadata, so a clip with missing or
/// unparseable fields is still importable.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaInfo {
    /// First video stream's `codec_name` (e.g. `"hevc"`, `"h264"`).
    pub codec_name: Option<String>,
    /// First video stream pixel width.
    pub width: Option<u32>,
    /// First video stream pixel height.
    pub height: Option<u32>,
    /// Container duration in seconds (`format.duration`, falling back to the
    /// first video stream's `duration`). `f64` because this is a display-only
    /// boundary value — clips are copied, not retimed, in this phase.
    pub duration_s: Option<f64>,
    /// First video stream's `r_frame_rate` parsed as an exact ratio.
    pub fps: Option<Rational>,
}

/// Parse ffprobe JSON into a [`MediaInfo`].
///
/// # Errors
/// Returns [`Error::Probe`] when `json` is not valid JSON. Missing or malformed
/// individual fields degrade to `None` rather than erroring.
pub fn parse_probe(json: &str) -> Result<MediaInfo> {
    let root: Value = serde_json::from_str(json).map_err(|e| Error::Probe(e.to_string()))?;
    let streams = root.get("streams").and_then(Value::as_array);
    let video = streams.and_then(|s| {
        s.iter()
            .find(|st| st.get("codec_type").and_then(Value::as_str) == Some("video"))
    });

    let codec_name = video
        .and_then(|v| v.get("codec_name"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let width = video.and_then(|v| v.get("width")).and_then(Value::as_u64).map(|n| n as u32);
    let height = video.and_then(|v| v.get("height")).and_then(Value::as_u64).map(|n| n as u32);
    let fps = video
        .and_then(|v| v.get("r_frame_rate"))
        .and_then(Value::as_str)
        .and_then(parse_ratio);

    let duration_s = root
        .get("format")
        .and_then(|f| f.get("duration"))
        .and_then(Value::as_str)
        .or_else(|| video.and_then(|v| v.get("duration")).and_then(Value::as_str))
        .and_then(|s| s.parse::<f64>().ok());

    Ok(MediaInfo { codec_name, width, height, duration_s, fps })
}

/// Parse an ffprobe `"num/den"` frame-rate string into a [`Rational`]. Returns
/// `None` for a zero denominator or a `0/0` (unknown) rate.
fn parse_ratio(s: &str) -> Option<Rational> {
    let (num, den) = s.split_once('/')?;
    let num: i64 = num.trim().parse().ok()?;
    let den: u32 = den.trim().parse().ok()?;
    if den == 0 || num == 0 {
        return None;
    }
    Some(Rational { num, den })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_h264_4k60_stream_and_duration() {
        let json = include_str!("../tests/fixtures/ffprobe/xavc-hs-4k60.json");
        let info = parse_probe(json).unwrap();
        assert_eq!(info.width, Some(3840));
        assert_eq!(info.height, Some(2160));
        assert_eq!(info.codec_name.as_deref(), Some("h264"));
        assert!(info.duration_s.unwrap() > 1.9 && info.duration_s.unwrap() < 2.1);
    }

    #[test]
    fn parses_drop_frame_rate_as_exact_ratio() {
        let json = include_str!("../tests/fixtures/ffprobe/df-2997.json");
        let info = parse_probe(json).unwrap();
        assert_eq!(info.fps, Some(Rational { num: 30000, den: 1001 }));
    }

    #[test]
    fn invalid_json_is_a_probe_error() {
        assert!(matches!(parse_probe("not json"), Err(Error::Probe(_))));
    }

    #[test]
    fn missing_fields_degrade_to_none_not_error() {
        let info = parse_probe(r#"{"streams":[],"format":{}}"#).unwrap();
        assert_eq!(info, MediaInfo { codec_name: None, width: None, height: None, duration_s: None, fps: None });
    }

    #[test]
    fn zero_denominator_rate_is_none() {
        assert_eq!(parse_ratio("0/0"), None);
    }
}
```

- [ ] **Step 3: Wire the module** — add `pub mod ffprobe;` to `crates/katto-engine/src/lib.rs`.

- [ ] **Step 4: Run the tests** — Run: `cargo test -p katto-engine ffprobe`. Expected: all 5 PASS. (If `xavc-hs-4k60.json` reports a `codec_name` other than `"h264"`, update the assertion to the captured value — the fixture is the source of truth.)

- [ ] **Step 5: Commit**

```bash
git add crates/katto-engine/src/ffprobe.rs crates/katto-engine/src/lib.rs crates/katto-engine/tests/fixtures/ffprobe
git commit -m "feat(engine): parse ffprobe json into MediaInfo"
```

---

## Task 3: Card recognition over an in-memory tree

**Files:**
- Create: `crates/katto-engine/src/ingest.rs` (module parent + shared types)
- Create: `crates/katto-engine/src/ingest/recognize.rs`
- Create: `crates/katto-engine/tests/fixtures/cards/{sony,generic-dcim,iphone-dcim,not-a-card}/...`
- Create: `crates/katto-engine/tests/recognize.rs`
- Modify: `crates/katto-engine/src/lib.rs`

**Interfaces:**
- Produces: `CardKind { Sony, GenericDcim, IphoneDcim }`; `VolumeTree { entries: Vec<PathBuf> }` (all dir+file paths relative to the volume root); `Card { kind: CardKind, clip_roots: Vec<PathBuf> }`; `recognize(tree: &VolumeTree) -> Option<Card>`.

- [ ] **Step 1: Build the fixture trees** — Run (real directory trees with tiny placeholder files):

```bash
cd crates/katto-engine && mkdir -p tests/fixtures/cards
# Sony ZV-E10 II XAVC layout
mkdir -p tests/fixtures/cards/sony/PRIVATE/M4ROOT/CLIP tests/fixtures/cards/sony/PRIVATE/M4ROOT/SUB
: > tests/fixtures/cards/sony/PRIVATE/M4ROOT/CLIP/C0001.MP4
: > tests/fixtures/cards/sony/PRIVATE/M4ROOT/CLIP/C0001M01.XML
: > tests/fixtures/cards/sony/PRIVATE/M4ROOT/CLIP/C0002.MP4
: > tests/fixtures/cards/sony/PRIVATE/M4ROOT/SUB/C0001S01.MP4
# Generic DCIM
mkdir -p tests/fixtures/cards/generic-dcim/DCIM/100MEDIA
: > tests/fixtures/cards/generic-dcim/DCIM/100MEDIA/MVI_0001.MOV
: > tests/fixtures/cards/generic-dcim/DCIM/100MEDIA/MVI_0001.THM
# iPhone DCIM
mkdir -p tests/fixtures/cards/iphone-dcim/DCIM/100APPLE
: > tests/fixtures/cards/iphone-dcim/DCIM/100APPLE/IMG_0001.MOV
: > tests/fixtures/cards/iphone-dcim/DCIM/100APPLE/IMG_0002.M4V
# Not a camera card
mkdir -p tests/fixtures/cards/not-a-card/Documents
: > tests/fixtures/cards/not-a-card/Documents/notes.txt
```

- [ ] **Step 2: Write the shared types + a failing unit test** — in `crates/katto-engine/src/ingest.rs`:

```rust
//! Pure SD-ingest logic: card recognition, clip enumeration/grouping,
//! rename/sequence math, and post-copy verification. Every function here is
//! pure over in-memory representations — the filesystem walk, the ffprobe
//! spawn, and the byte copy all live in the app crate.

use std::path::PathBuf;

pub mod enumerate;
pub mod naming;
pub mod recognize;
pub mod verify;

/// The kind of camera card recognized, by on-card layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardKind {
    /// Sony `PRIVATE/M4ROOT/CLIP/` (ZV-E10 II XAVC HS/S).
    Sony,
    /// Generic `DCIM/100MEDIA`-style camera card.
    GenericDcim,
    /// iPhone `DCIM/100APPLE`-style card.
    IphoneDcim,
}

/// A flat, in-memory listing of a volume's directory tree: every directory and
/// file path, relative to the volume mount root. Built by the app crate's walk.
#[derive(Debug, Clone, Default)]
pub struct VolumeTree {
    /// All entries (dirs and files), relative to the volume root.
    pub entries: Vec<PathBuf>,
}

/// The result of recognizing a volume as a camera card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    /// Which card layout matched.
    pub kind: CardKind,
    /// Directories (relative to the volume root) to enumerate clips from.
    pub clip_roots: Vec<PathBuf>,
}

/// A single file discovered under a clip root, with its byte size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    /// Path relative to the volume root.
    pub path: PathBuf,
    /// File size in bytes.
    pub size: u64,
}

/// One clip in an enumerated group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipEntry {
    /// Source path relative to the volume root.
    pub path: PathBuf,
    /// File name (final path component).
    pub name: String,
    /// Byte size.
    pub size: u64,
    /// Lowercased extension without the dot (e.g. `"mp4"`).
    pub ext: String,
    /// True for a video file (importable), false for a sidecar/thumbnail.
    pub is_video: bool,
    /// Whether the clip is selected by default (videos yes, sidecars no).
    pub selected: bool,
}

/// A group of clips sharing card substructure (e.g. `CLIP`, `SUB`, `100APPLE`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipGroup {
    /// The group label (the substructure directory name).
    pub label: String,
    /// The clips in this group, in stable order.
    pub clips: Vec<ClipEntry>,
}
```

then in `crates/katto-engine/src/ingest/recognize.rs`:

```rust
//! Card recognition: classify a volume by the marker directories it contains.

use std::path::{Path, PathBuf};

use crate::ingest::{Card, CardKind, VolumeTree};

/// Recognize a volume as a camera card, or `None` for a non-camera volume.
///
/// A volume is a camera card iff it contains Sony `PRIVATE/M4ROOT/CLIP/` **or**
/// a `DCIM/` tree. Sony wins if both are present. For `DCIM/`, a subfolder
/// matching `NNNAPPLE` marks an iPhone card; otherwise it is generic. Clip roots
/// are the `CLIP`/`SUB` dirs (Sony) or the immediate `DCIM/` subfolders.
pub fn recognize(tree: &VolumeTree) -> Option<Card> {
    let has = |p: &str| tree.entries.iter().any(|e| e == Path::new(p));

    if has("PRIVATE/M4ROOT/CLIP") {
        let mut clip_roots = vec![PathBuf::from("PRIVATE/M4ROOT/CLIP")];
        if has("PRIVATE/M4ROOT/SUB") {
            clip_roots.push(PathBuf::from("PRIVATE/M4ROOT/SUB"));
        }
        return Some(Card { kind: CardKind::Sony, clip_roots });
    }

    let dcim_subdirs: Vec<PathBuf> = tree
        .entries
        .iter()
        .filter(|e| e.parent() == Some(Path::new("DCIM")))
        .filter(|e| is_dir_marker(tree, e))
        .cloned()
        .collect();

    if has("DCIM") && !dcim_subdirs.is_empty() {
        let iphone = dcim_subdirs
            .iter()
            .any(|d| d.file_name().and_then(|n| n.to_str()).is_some_and(is_apple_dir));
        let kind = if iphone { CardKind::IphoneDcim } else { CardKind::GenericDcim };
        return Some(Card { kind, clip_roots: dcim_subdirs });
    }

    None
}

/// True when `entry` is an `DCIM/NNNAPPLE` directory (three digits then `APPLE`).
fn is_apple_dir(name: &str) -> bool {
    name.len() == 8 && name.ends_with("APPLE") && name[..3].chars().all(|c| c.is_ascii_digit())
}

/// A `DCIM` child counts as a clip-root dir if the tree contains any entry
/// nested beneath it (a real card always has media under the subfolder).
fn is_dir_marker(tree: &VolumeTree, dir: &Path) -> bool {
    tree.entries.iter().any(|e| e.parent() == Some(dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(paths: &[&str]) -> VolumeTree {
        VolumeTree { entries: paths.iter().map(PathBuf::from).collect() }
    }

    #[test]
    fn sony_layout_is_recognized_with_clip_and_sub_roots() {
        let t = tree(&[
            "PRIVATE", "PRIVATE/M4ROOT", "PRIVATE/M4ROOT/CLIP", "PRIVATE/M4ROOT/SUB",
            "PRIVATE/M4ROOT/CLIP/C0001.MP4", "PRIVATE/M4ROOT/SUB/C0001S01.MP4",
        ]);
        let card = recognize(&t).unwrap();
        assert_eq!(card.kind, CardKind::Sony);
        assert_eq!(card.clip_roots, vec![
            PathBuf::from("PRIVATE/M4ROOT/CLIP"),
            PathBuf::from("PRIVATE/M4ROOT/SUB"),
        ]);
    }

    #[test]
    fn iphone_dcim_is_recognized() {
        let t = tree(&["DCIM", "DCIM/100APPLE", "DCIM/100APPLE/IMG_0001.MOV"]);
        let card = recognize(&t).unwrap();
        assert_eq!(card.kind, CardKind::IphoneDcim);
        assert_eq!(card.clip_roots, vec![PathBuf::from("DCIM/100APPLE")]);
    }

    #[test]
    fn generic_dcim_is_recognized() {
        let t = tree(&["DCIM", "DCIM/100MEDIA", "DCIM/100MEDIA/MVI_0001.MOV"]);
        assert_eq!(recognize(&t).unwrap().kind, CardKind::GenericDcim);
    }

    #[test]
    fn non_camera_volume_is_none() {
        let t = tree(&["Documents", "Documents/notes.txt"]);
        assert!(recognize(&t).is_none());
    }

    #[test]
    fn sony_wins_when_both_markers_present() {
        let t = tree(&[
            "DCIM", "DCIM/100APPLE", "DCIM/100APPLE/IMG.MOV",
            "PRIVATE/M4ROOT/CLIP", "PRIVATE/M4ROOT/CLIP/C0001.MP4",
        ]);
        assert_eq!(recognize(&t).unwrap().kind, CardKind::Sony);
    }
}
```

- [ ] **Step 3: Wire the module** — add `pub mod ingest;` to `crates/katto-engine/src/lib.rs`.

- [ ] **Step 4: Run the unit tests** — Run: `cargo test -p katto-engine ingest::recognize`. Expected: 5 PASS.

- [ ] **Step 5: Write the fixture-tree integration test** — in `crates/katto-engine/tests/recognize.rs` (walks the real trees so the walk-shape the app crate must produce is exercised):

```rust
use std::path::{Path, PathBuf};

use katto_engine::ingest::{recognize::recognize, CardKind, VolumeTree};

/// Walk a fixture directory into a `VolumeTree` of paths relative to `root` —
/// the same in-memory shape the app crate's real walk produces.
fn walk(root: &Path) -> VolumeTree {
    fn rec(base: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            out.push(path.strip_prefix(base).unwrap().to_path_buf());
            if path.is_dir() {
                rec(base, &path, out);
            }
        }
    }
    let mut entries = Vec::new();
    rec(root, root, &mut entries);
    VolumeTree { entries }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cards").join(name)
}

#[test]
fn sony_fixture_tree_is_a_sony_card() {
    let card = recognize(&walk(&fixture("sony"))).unwrap();
    assert_eq!(card.kind, CardKind::Sony);
    assert!(card.clip_roots.contains(&PathBuf::from("PRIVATE/M4ROOT/CLIP")));
}

#[test]
fn iphone_fixture_tree_is_an_iphone_card() {
    assert_eq!(recognize(&walk(&fixture("iphone-dcim"))).unwrap().kind, CardKind::IphoneDcim);
}

#[test]
fn not_a_card_fixture_is_none() {
    assert!(recognize(&walk(&fixture("not-a-card"))).is_none());
}
```

- [ ] **Step 6: Run the integration test** — Run: `cargo test -p katto-engine --test recognize`. Expected: 3 PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/katto-engine/src/ingest.rs crates/katto-engine/src/ingest/recognize.rs crates/katto-engine/src/lib.rs crates/katto-engine/tests/recognize.rs crates/katto-engine/tests/fixtures/cards
git commit -m "feat(engine): recognize sony/dcim/iphone camera cards"
```

---

## Task 4: Clip enumeration + grouping

**Files:**
- Create: `crates/katto-engine/src/ingest/enumerate.rs`
- Create: `crates/katto-engine/tests/enumerate.rs`

**Interfaces:**
- Consumes: `CardKind`, `FileEntry`, `ClipEntry`, `ClipGroup` from `ingest.rs`.
- Produces: `enumerate(kind: CardKind, files: &[FileEntry]) -> Vec<ClipGroup>`; `VIDEO_EXTS: [&str; 4]`.

- [ ] **Step 1: Write the failing test** — in `crates/katto-engine/src/ingest/enumerate.rs`:

```rust
//! Clip enumeration: classify files by extension and group by card substructure.

use std::path::Path;

use crate::ingest::{CardKind, ClipEntry, ClipGroup, FileEntry};

/// Video extensions katto imports (compared case-insensitively).
pub const VIDEO_EXTS: [&str; 4] = ["mp4", "mov", "mts", "m4v"];

/// Group and classify the files walked from a card's clip roots.
///
/// Videos (`VIDEO_EXTS`) are selected by default; everything else (sidecars such
/// as `.xml`/`.thm`) is listed but deselected. Files are grouped by the name of
/// their immediate parent directory (`CLIP`, `SUB`, `100APPLE`, …). Groups and
/// clips are returned in stable, path-sorted order so a plan is deterministic.
pub fn enumerate(_kind: CardKind, files: &[FileEntry]) -> Vec<ClipGroup> {
    let mut sorted: Vec<&FileEntry> = files.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));

    let mut groups: Vec<ClipGroup> = Vec::new();
    for f in sorted {
        let name = f.path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
        let ext = f
            .path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();
        let is_video = VIDEO_EXTS.contains(&ext.as_str());
        let label = f
            .path
            .parent()
            .and_then(Path::file_name)
            .and_then(|n| n.to_str())
            .unwrap_or("clips")
            .to_string();

        let clip = ClipEntry { path: f.path.clone(), name, size: f.size, ext, is_video, selected: is_video };
        match groups.iter_mut().find(|g| g.label == label) {
            Some(g) => g.clips.push(clip),
            None => groups.push(ClipGroup { label, clips: vec![clip] }),
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fe(path: &str, size: u64) -> FileEntry {
        FileEntry { path: PathBuf::from(path), size }
    }

    #[test]
    fn videos_selected_sidecars_deselected() {
        let groups = enumerate(CardKind::Sony, &[
            fe("PRIVATE/M4ROOT/CLIP/C0001.MP4", 100),
            fe("PRIVATE/M4ROOT/CLIP/C0001M01.XML", 5),
        ]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].label, "CLIP");
        let mp4 = groups[0].clips.iter().find(|c| c.name == "C0001.MP4").unwrap();
        let xml = groups[0].clips.iter().find(|c| c.name == "C0001M01.XML").unwrap();
        assert!(mp4.is_video && mp4.selected);
        assert!(!xml.is_video && !xml.selected);
    }

    #[test]
    fn groups_by_substructure_clip_and_sub() {
        let groups = enumerate(CardKind::Sony, &[
            fe("PRIVATE/M4ROOT/CLIP/C0001.MP4", 1),
            fe("PRIVATE/M4ROOT/SUB/C0001S01.MP4", 1),
        ]);
        let labels: Vec<&str> = groups.iter().map(|g| g.label.as_str()).collect();
        assert_eq!(labels, vec!["CLIP", "SUB"]);
    }

    #[test]
    fn extension_case_is_normalized() {
        let groups = enumerate(CardKind::IphoneDcim, &[fe("DCIM/100APPLE/IMG.MoV", 1)]);
        assert_eq!(groups[0].clips[0].ext, "mov");
        assert!(groups[0].clips[0].is_video);
    }
}
```

- [ ] **Step 2: Run the tests** — Run: `cargo test -p katto-engine ingest::enumerate`. Expected: 3 PASS.

- [ ] **Step 3: Write the fixture integration test** — in `crates/katto-engine/tests/enumerate.rs`:

```rust
use std::path::{Path, PathBuf};

use katto_engine::ingest::{enumerate::enumerate, CardKind, FileEntry};

fn walk_files(root: &Path) -> Vec<FileEntry> {
    fn rec(base: &Path, dir: &Path, out: &mut Vec<FileEntry>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                rec(base, &path, out);
            } else {
                let size = entry.metadata().unwrap().len();
                out.push(FileEntry { path: path.strip_prefix(base).unwrap().to_path_buf(), size });
            }
        }
    }
    let mut out = Vec::new();
    rec(root, root, &mut out);
    out
}

#[test]
fn sony_fixture_enumerates_clip_and_sub_groups() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cards/sony");
    let groups = enumerate(CardKind::Sony, &walk_files(&root));
    let labels: Vec<&str> = groups.iter().map(|g| g.label.as_str()).collect();
    assert!(labels.contains(&"CLIP") && labels.contains(&"SUB"));
    let clip = groups.iter().find(|g| g.label == "CLIP").unwrap();
    assert!(clip.clips.iter().any(|c| c.name == "C0001.MP4" && c.selected));
    assert!(clip.clips.iter().any(|c| c.name == "C0001M01.XML" && !c.selected));
    let _ = PathBuf::new();
}
```

- [ ] **Step 4: Run it** — Run: `cargo test -p katto-engine --test enumerate`. Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/katto-engine/src/ingest/enumerate.rs crates/katto-engine/tests/enumerate.rs
git commit -m "feat(engine): enumerate and group card clips by substructure"
```

---

## Task 5: Rename / sequence math

`YYYY-MM-DD_NNN.ext`: date passed in by the caller (project shoot date or today — the engine stays clock-free); `NNN` is a zero-padded 3-digit sequence continuing from the highest existing `NNN` for that date in `footage/`. Collisions impossible by construction.

**Files:**
- Create: `crates/katto-engine/src/ingest/naming.rs`

**Interfaces:**
- Consumes: `Rename` from `ingest.rs` (add the struct in Step 1).
- Produces: `next_sequence(date: &str, existing: &[String]) -> u32`; `dest_filename(date: &str, seq: u32, ext: &str) -> String`; `plan_renames(date: &str, existing: &[String], sources: &[(PathBuf, String)]) -> Vec<Rename>` where each source is `(source_path, lowercased_ext)`.

- [ ] **Step 1: Add the `Rename` type** — in `crates/katto-engine/src/ingest.rs`, add:

```rust
/// A planned copy: a source path and the `YYYY-MM-DD_NNN.ext` name it lands as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rename {
    /// Source path relative to the volume root.
    pub source: PathBuf,
    /// Destination file name inside the project's `footage/`.
    pub dest_name: String,
}
```

- [ ] **Step 2: Write the failing test** — in `crates/katto-engine/src/ingest/naming.rs`:

```rust
//! Deterministic footage renaming: `YYYY-MM-DD_NNN.ext` with a per-date
//! sequence continuing from what already exists in `footage/`.

use std::path::PathBuf;

use crate::ingest::Rename;

/// The next 3-digit sequence for `date`, continuing from the highest existing
/// `YYYY-MM-DD_NNN.*` name in `existing`. Names for other dates are ignored.
/// Returns `1` when none exist for the date.
pub fn next_sequence(date: &str, existing: &[String]) -> u32 {
    let prefix = format!("{date}_");
    existing
        .iter()
        .filter_map(|name| {
            let rest = name.strip_prefix(&prefix)?;
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            if digits.len() == 3 { digits.parse::<u32>().ok() } else { None }
        })
        .max()
        .map_or(1, |n| n + 1)
}

/// Format one destination file name. `ext` must already be lowercased and
/// dot-less (e.g. `"mp4"`).
pub fn dest_filename(date: &str, seq: u32, ext: &str) -> String {
    format!("{date}_{seq:03}.{ext}")
}

/// Plan the renames for a batch of sources, assigning consecutive sequence
/// numbers in source order (sources should be pre-sorted by the caller for
/// determinism). `sources` pairs each source path with its lowercased, dot-less
/// extension.
pub fn plan_renames(date: &str, existing: &[String], sources: &[(PathBuf, String)]) -> Vec<Rename> {
    let start = next_sequence(date, existing);
    sources
        .iter()
        .enumerate()
        .map(|(i, (source, ext))| Rename {
            source: source.clone(),
            dest_name: dest_filename(date, start + i as u32, ext),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_sequence_for_a_date_is_one() {
        assert_eq!(next_sequence("2026-07-22", &[]), 1);
    }

    #[test]
    fn sequence_continues_from_highest_existing_for_that_date() {
        let existing = vec![
            "2026-07-22_001.mp4".to_string(),
            "2026-07-22_004.mov".to_string(),
            "2026-07-21_009.mp4".to_string(), // other date — ignored
        ];
        assert_eq!(next_sequence("2026-07-22", &existing), 5);
    }

    #[test]
    fn dest_filename_zero_pads_to_three_digits() {
        assert_eq!(dest_filename("2026-07-22", 7, "mp4"), "2026-07-22_007.mp4");
    }

    #[test]
    fn plan_assigns_consecutive_names_preserving_extension() {
        let existing = vec!["2026-07-22_002.mp4".to_string()];
        let sources = vec![
            (PathBuf::from("CLIP/C0001.MP4"), "mp4".to_string()),
            (PathBuf::from("CLIP/C0002.MOV"), "mov".to_string()),
        ];
        let plan = plan_renames("2026-07-22", &existing, &sources);
        assert_eq!(plan[0].dest_name, "2026-07-22_003.mp4");
        assert_eq!(plan[1].dest_name, "2026-07-22_004.mov");
    }
}
```

- [ ] **Step 3: Run the tests** — Run: `cargo test -p katto-engine ingest::naming`. Expected: 4 PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/katto-engine/src/ingest.rs crates/katto-engine/src/ingest/naming.rs
git commit -m "feat(engine): plan footage renames with per-date sequence"
```

---

## Task 6: Post-copy verification comparator

**Files:**
- Create: `crates/katto-engine/src/ingest/verify.rs`

**Interfaces:**
- Consumes: `VerifyError` from `ingest.rs` (add in Step 1).
- Produces: `verify(expected: &[(String, u64)], actual: &[(String, u64)]) -> Vec<VerifyError>` (empty vec = all good). Inputs are `(dest_name, byte_size)` pairs — expected = source sizes, actual = copied-file sizes.

- [ ] **Step 1: Add the `VerifyError` type** — in `crates/katto-engine/src/ingest.rs`, add:

```rust
/// A verification failure between the expected copy set and what landed on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    /// The number of copied files differs from the selection.
    CountMismatch {
        /// Files that should have been copied.
        expected: usize,
        /// Files actually present.
        actual: usize,
    },
    /// A copied file's byte size differs from its source.
    SizeMismatch {
        /// Destination file name.
        name: String,
        /// Source byte count.
        expected: u64,
        /// Copied byte count.
        actual: u64,
    },
    /// An expected file is missing from the destination entirely.
    Missing {
        /// Destination file name.
        name: String,
    },
}
```

- [ ] **Step 2: Write the failing test** — in `crates/katto-engine/src/ingest/verify.rs`:

```rust
//! Post-copy verification: file-count parity and per-file byte-size match.

use std::collections::HashMap;

use crate::ingest::VerifyError;

/// Compare the expected copy set against what landed on disk. Returns an empty
/// vec when every file is present at its exact source size; otherwise a list of
/// every discrepancy (count mismatch first, then per-file size/missing errors in
/// `expected` order). Inputs are `(dest_name, byte_size)` pairs.
pub fn verify(expected: &[(String, u64)], actual: &[(String, u64)]) -> Vec<VerifyError> {
    let mut errors = Vec::new();
    if expected.len() != actual.len() {
        errors.push(VerifyError::CountMismatch { expected: expected.len(), actual: actual.len() });
    }
    let actual_by_name: HashMap<&str, u64> = actual.iter().map(|(n, s)| (n.as_str(), *s)).collect();
    for (name, expected_size) in expected {
        match actual_by_name.get(name.as_str()) {
            None => errors.push(VerifyError::Missing { name: name.clone() }),
            Some(&actual_size) if actual_size != *expected_size => {
                errors.push(VerifyError::SizeMismatch { name: name.clone(), expected: *expected_size, actual: actual_size });
            }
            Some(_) => {}
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(name: &str, size: u64) -> (String, u64) {
        (name.to_string(), size)
    }

    #[test]
    fn identical_sets_have_no_errors() {
        let e = vec![pair("a.mp4", 100), pair("b.mov", 200)];
        assert!(verify(&e, &e).is_empty());
    }

    #[test]
    fn size_mismatch_is_reported() {
        let e = vec![pair("a.mp4", 100)];
        let a = vec![pair("a.mp4", 99)];
        assert_eq!(verify(&e, &a), vec![VerifyError::SizeMismatch { name: "a.mp4".to_string(), expected: 100, actual: 99 }]);
    }

    #[test]
    fn missing_file_reports_count_and_missing() {
        let e = vec![pair("a.mp4", 100), pair("b.mov", 200)];
        let a = vec![pair("a.mp4", 100)];
        let errors = verify(&e, &a);
        assert!(errors.contains(&VerifyError::CountMismatch { expected: 2, actual: 1 }));
        assert!(errors.contains(&VerifyError::Missing { name: "b.mov".to_string() }));
    }
}
```

- [ ] **Step 3: Run the tests** — Run: `cargo test -p katto-engine ingest::verify`. Expected: 3 PASS.

- [ ] **Step 4: Full engine gate** — Run: `cargo test -p katto-engine`. Expected: every ingest + ffprobe + error test PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/katto-engine/src/ingest.rs crates/katto-engine/src/ingest/verify.rs
git commit -m "feat(engine): verify copied footage by count and byte size"
```

---

## Task 7: ffprobe spawn site (app crate)

The single ffprobe subprocess call, kept thin; the parse is the engine's. Covered by an `#[ignore]`d integration test (needs the `ffprobe` binary + a real sample) per the testing rule.

**Files:**
- Create: `src-tauri/src/ffprobe.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod ffprobe;`)

**Interfaces:**
- Produces: `probe_clip(path: &Path) -> Result<katto_engine::ffprobe::MediaInfo>`; `ffprobe_argv(path: &Path) -> Vec<String>` (pure, tested).

- [ ] **Step 1: Write the pure-argv failing test + the spawn shell** — in `src-tauri/src/ffprobe.rs`:

```rust
use std::path::Path;
use std::process::Command;

use katto_engine::ffprobe::{parse_probe, MediaInfo};

use crate::error::{Error, Result};

/// Build the ffprobe argument vector for `path`. Pure and deterministic (pinned
/// flags, no clock/RNG) so it is unit-tested without spawning. Mirrors the
/// hyper-frames `cut-video` probe but uses `-show_streams -show_format` per the
/// Phase-3 PRD.
fn ffprobe_argv(path: &Path) -> Vec<String> {
    vec![
        "-loglevel".to_string(), "error".to_string(),
        "-print_format".to_string(), "json".to_string(),
        "-show_streams".to_string(),
        "-show_format".to_string(),
        path.to_string_lossy().into_owned(),
    ]
}

/// Probe one clip's metadata by spawning `ffprobe`. The single spawn site; the
/// JSON parse is the engine's pure `parse_probe`.
///
/// # Errors
/// [`Error::Io`] if ffprobe cannot be spawned; [`Error::Engine`] if its output
/// does not parse.
pub fn probe_clip(path: &Path) -> Result<MediaInfo> {
    let output = Command::new("ffprobe").args(ffprobe_argv(path)).output()?;
    if !output.status.success() {
        return Err(Error::Io(format!(
            "ffprobe failed for {}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let json = String::from_utf8_lossy(&output.stdout);
    Ok(parse_probe(&json)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_pins_json_streams_and_format_flags() {
        let argv = ffprobe_argv(Path::new("/tmp/C0001.MP4"));
        assert!(argv.contains(&"-show_streams".to_string()));
        assert!(argv.contains(&"-show_format".to_string()));
        assert_eq!(argv.last().unwrap(), "/tmp/C0001.MP4");
    }

    #[test]
    #[ignore = "hardware: needs ffprobe binary + a real sample file"]
    fn probe_clip_reads_real_sample() {
        // Manual: point at a real clip and assert duration_s.is_some().
        let info = probe_clip(Path::new("/tmp/s.mp4")).unwrap();
        assert!(info.duration_s.is_some());
    }
}
```

- [ ] **Step 2: Wire the module** — add `mod ffprobe;` to `src-tauri/src/lib.rs` (module list near the top).

- [ ] **Step 3: Run the pure test** — Run: `cargo test -p katto ffprobe::tests::argv_pins_json_streams_and_format_flags`. Expected: PASS. (The `#[ignore]`d test does not run under `just check`.)

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/ffprobe.rs src-tauri/src/lib.rs
git commit -m "feat(ingest): add ffprobe spawn site over the engine parser"
```

---

## Task 8: Copy job (copy → verify → rename, quarantine, events)

The filesystem worker, spawned through the Phase-1 `JobRuntime`. Copy strategy (satisfies "card never written; copy-only; nothing fails silently; quarantine on failure"): for each planned rename, copy `source` → `footage/<dest>.partial`, stat its size vs the source, and on match `rename` `.partial` → `footage/<dest>`; on any copy error or size mismatch, leave the `.partial` in place as quarantine, and fail the job. The source is only ever opened for reading. On overall success, write the `ingested` events row.

**Files:**
- Create: `src-tauri/src/ingest.rs` + `src-tauri/src/ingest/copy.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod ingest;`)

**Interfaces:**
- Consumes: `katto_engine::ingest::{Rename, verify::verify}`, `JobContext`, `DbHandle`, `crate::db::events`.
- Produces: `CopyPlan { footage_dir: PathBuf, renames: Vec<Rename>, source_root: PathBuf, project_slug: String }`; `async fn run_copy_job(ctx: JobContext, db: DbHandle, app: AppHandle, plan: CopyPlan) -> std::result::Result<(), String>`; `copy_one(source: &Path, footage_dir: &Path, dest_name: &str) -> std::io::Result<u64>` (returns copied byte count; pure-ish, tempdir-tested).

- [ ] **Step 1: Write the module parent** — `src-tauri/src/ingest.rs`:

```rust
//! The ingest copy job: the only filesystem-touching part of SD ingest. Pure
//! recognition/enumeration/naming/verification live in `katto_engine::ingest`.

pub mod copy;
```

- [ ] **Step 2: Write the failing tempdir integration tests + the worker** — `src-tauri/src/ingest/copy.rs`:

```rust
use std::path::{Path, PathBuf};

use tauri::AppHandle;

use katto_engine::ingest::verify::verify;
use katto_engine::ingest::Rename;

use crate::broadcast;
use crate::db::{events, DbHandle};
use crate::jobs::JobContext;

/// Everything the copy job needs, resolved by the command before spawning.
pub struct CopyPlan {
    /// Absolute path to the volume/source root that `Rename::source` is relative to.
    pub source_root: PathBuf,
    /// Absolute path to the project's `footage/` directory.
    pub footage_dir: PathBuf,
    /// The planned copies.
    pub renames: Vec<Rename>,
    /// The owning project slug (for the events row).
    pub project_slug: String,
}

/// Copy one source file to `footage/<dest_name>.partial`, then rename to the
/// final name only after the byte count matches. Returns the copied byte count.
/// The source is opened read-only by `std::fs::copy`; the card is never written.
pub fn copy_one(source: &Path, footage_dir: &Path, dest_name: &str) -> std::io::Result<u64> {
    let partial = footage_dir.join(format!("{dest_name}.partial"));
    let final_path = footage_dir.join(dest_name);
    let copied = std::fs::copy(source, &partial)?;
    let source_size = std::fs::metadata(source)?.len();
    if copied != source_size {
        // Leave the `.partial` as quarantine; do not rename into the final name.
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("size mismatch for {dest_name}: {copied} != {source_size}"),
        ));
    }
    std::fs::rename(&partial, &final_path)?;
    Ok(copied)
}

/// Run the copy job: per-file copy+verify with progress ticks, a final
/// count/size verification, and an `ingested` events row on success. Returns
/// `Err(message)` for the jobs runtime to record as a terminal failure.
pub async fn run_copy_job(
    ctx: JobContext,
    db: DbHandle,
    app: AppHandle,
    plan: CopyPlan,
) -> std::result::Result<(), String> {
    let total = plan.renames.len();
    let mut expected: Vec<(String, u64)> = Vec::with_capacity(total);
    let mut actual: Vec<(String, u64)> = Vec::with_capacity(total);
    let mut total_bytes: u64 = 0;

    for (i, rename) in plan.renames.iter().enumerate() {
        let source = plan.source_root.join(&rename.source);
        let source_size = std::fs::metadata(&source).map(|m| m.len()).unwrap_or(0);
        ctx.progress(i as f64 / total as f64, Some(format!("Copying {}", rename.dest_name))).await;

        let footage = plan.footage_dir.clone();
        let dest = rename.dest_name.clone();
        let src = source.clone();
        let copied = tauri::async_runtime::spawn_blocking(move || copy_one(&src, &footage, &dest))
            .await
            .map_err(|_| "copy task panicked".to_string())?
            .map_err(|e| e.to_string())?;

        expected.push((rename.dest_name.clone(), source_size));
        actual.push((rename.dest_name.clone(), copied));
        total_bytes += copied;
    }

    let errors = verify(&expected, &actual);
    if !errors.is_empty() {
        return Err(format!("verification failed: {errors:?}"));
    }

    let payload = serde_json::json!({ "count": total, "bytes": total_bytes, "project": plan.project_slug }).to_string();
    let slug = plan.project_slug.clone();
    let _ = db
        .call(move |conn| events::record(conn, "ingested", Some(&slug), Some(&payload)))
        .await;
    broadcast::events_appended(&app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_one_renames_partial_to_final_on_success() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("C0001.MP4");
        std::fs::write(&src, b"footage-bytes").unwrap();
        let footage = dir.path().join("footage");
        std::fs::create_dir_all(&footage).unwrap();

        let copied = copy_one(&src, &footage, "2026-07-22_001.mp4").unwrap();
        assert_eq!(copied, 13);
        assert!(footage.join("2026-07-22_001.mp4").exists());
        assert!(!footage.join("2026-07-22_001.mp4.partial").exists());
        // Source untouched.
        assert_eq!(std::fs::read(&src).unwrap(), b"footage-bytes");
    }

    #[test]
    fn copy_one_missing_source_leaves_no_final_file() {
        let dir = tempfile::tempdir().unwrap();
        let footage = dir.path().join("footage");
        std::fs::create_dir_all(&footage).unwrap();
        let err = copy_one(&dir.path().join("nope.mp4"), &footage, "2026-07-22_001.mp4");
        assert!(err.is_err());
        assert!(!footage.join("2026-07-22_001.mp4").exists());
    }
}
```

- [ ] **Step 3: Wire the module** — add `mod ingest;` to `src-tauri/src/lib.rs`.

- [ ] **Step 4: Run the tempdir tests** — Run: `cargo test -p katto ingest::copy`. Expected: 2 PASS. (`tempfile` is already a `dev-dependency`.)

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/ingest.rs src-tauri/src/ingest/copy.rs src-tauri/src/lib.rs
git commit -m "feat(ingest): copy job with partial-quarantine verify and events row"
```

---

## Task 9: Volume watcher

Non-recursive `notify` watch on `/Volumes`. On a new mount: debounce until readable (poll the mount for its marker dirs up to ~1 s), walk the tree, `recognize`, and if it is a card, `enumerate`, probe durations, store the `CardOffer` in `IngestState`, broadcast `CardDetected`, and send the `katto://ingest` notification. On unmount: clear the offer if it matched, broadcast `CardRemoved`. Thin spawn/watch site; the pure `is_card_ready` gate is unit-tested, the watcher wiring is a manual hardware checkpoint.

**Files:**
- Modify: `src-tauri/Cargo.toml` (add `notify = "8.2"`)
- Create: `src-tauri/src/volumes.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod volumes;`)
- Modify: `src-tauri/src/state.rs` (add `IngestState`)
- Modify: `src-tauri/src/broadcast.rs` (add `CardDetected`, `CardRemoved`)
- Modify: `src-tauri/src/commands/ingest.rs` (defined in Task 10 — `CardOffer` type lives there; import it here)

> **Ordering note:** `CardOffer` (the wire DTO) is defined in Task 10's `commands/ingest.rs`. To keep this task compilable on its own, define `CardOffer` and its nested DTOs in `commands/ingest.rs` **first** (Task 10 Step 1), or temporarily place the struct here and move it in Task 10. The recommended order is: do Task 10 Step 1 (types only) before Task 9, then the rest of Task 10. The subagent executing this plan should read both tasks before starting Task 9.

**Interfaces:**
- Consumes: `katto_engine::ingest::{recognize::recognize, enumerate::enumerate, VolumeTree, FileEntry}`, `crate::ffprobe::probe_clip`, `CardOffer` (Task 10).
- Produces: `IngestState { current: std::sync::Mutex<Option<CardOffer>> }`; `start_watcher(app: AppHandle, studio_root_is_under_volumes: bool)`; `walk_volume(root: &Path) -> (VolumeTree, Vec<FileEntry>)`; `is_card_ready(tree: &VolumeTree) -> bool` (pure).

- [ ] **Step 1: Add the dependency** — Run: `cd src-tauri && cargo add notify@8.2`. Expected: `notify = "8.2"` (or `8.2.x`) added under `[dependencies]`. Confirm it resolves: `cargo build -p katto`.

- [ ] **Step 2: Add `IngestState`** — in `src-tauri/src/state.rs`, add (kept separate from `AppState` so the watcher, started in `setup`, can populate it via `app.state()`):

```rust
use std::sync::Mutex;

use crate::commands::ingest::CardOffer;

/// Holds the currently-detected card offer, if any. Populated by the volume
/// watcher, read by the `card_offer` command, cleared on unmount.
#[derive(Default)]
pub struct IngestState {
    pub current: Mutex<Option<CardOffer>>,
}
```

- [ ] **Step 3: Add the broadcast events** — in `src-tauri/src/broadcast.rs`, add:

```rust
/// Broadcast when a camera card is detected and enumerated.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
pub struct CardDetected {
    pub offer: crate::commands::ingest::CardOffer,
}

/// Broadcast when the detected card's volume is unmounted.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
pub struct CardRemoved;
```

and the emit helpers:

```rust
/// Best-effort, same contract as [`events_appended`].
pub fn card_detected(app: &AppHandle, offer: crate::commands::ingest::CardOffer) {
    let _ = CardDetected { offer }.emit(app);
}

/// Best-effort, same contract as [`events_appended`].
pub fn card_removed(app: &AppHandle) {
    let _ = CardRemoved.emit(app);
}
```

- [ ] **Step 4: Write the pure-gate failing test + the watcher shell** — `src-tauri/src/volumes.rs`:

```rust
use std::path::{Path, PathBuf};

use katto_engine::ingest::{recognize::recognize, VolumeTree};

/// True when the freshly-mounted volume already exposes a recognizable card
/// layout — used to debounce: a card that isn't ready yet returns `false` and
/// the watcher retries. Pure over the walked tree.
pub fn is_card_ready(tree: &VolumeTree) -> bool {
    recognize(tree).is_some()
}

/// Walk a mounted volume into the in-memory shapes the engine consumes: the full
/// `VolumeTree` (for recognition) and the per-file list with sizes (for
/// enumeration). Skips unreadable entries rather than failing the walk.
pub fn walk_volume(root: &Path) -> (VolumeTree, Vec<katto_engine::ingest::FileEntry>) {
    use katto_engine::ingest::FileEntry;
    let mut entries = Vec::new();
    let mut files = Vec::new();
    fn rec(base: &Path, dir: &Path, entries: &mut Vec<PathBuf>, files: &mut Vec<katto_engine::ingest::FileEntry>) {
        let Ok(read) = std::fs::read_dir(dir) else { return };
        for entry in read.flatten() {
            let path = entry.path();
            let Ok(rel) = path.strip_prefix(base) else { continue };
            entries.push(rel.to_path_buf());
            if path.is_dir() {
                rec(base, &path, entries, files);
            } else if let Ok(meta) = entry.metadata() {
                files.push(FileEntry { path: rel.to_path_buf(), size: meta.len() });
            }
        }
    }
    rec(root, root, &mut entries, &mut files);
    (VolumeTree { entries }, files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use katto_engine::ingest::VolumeTree;

    #[test]
    fn ready_when_tree_recognizes_as_card() {
        let tree = VolumeTree { entries: vec![PathBuf::from("PRIVATE/M4ROOT/CLIP"), PathBuf::from("PRIVATE/M4ROOT/CLIP/C0001.MP4")] };
        assert!(is_card_ready(&tree));
    }

    #[test]
    fn not_ready_when_empty() {
        assert!(!is_card_ready(&VolumeTree::default()));
    }

    #[test]
    fn walk_reads_a_fixture_tree_from_disk() {
        // Reuses the engine's committed Sony fixture via a relative path.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/katto-engine/tests/fixtures/cards/sony");
        if root.exists() {
            let (tree, files) = walk_volume(&root);
            assert!(is_card_ready(&tree));
            assert!(files.iter().any(|f| f.path.ends_with("C0001.MP4")));
        }
    }
}
```

- [ ] **Step 5: Add the watcher wiring** (below the tests-covered fns, in the same file) — the thin notify site. It is not unit-tested (single spawn/watch site per the testing rule; exercised by the manual hardware checkpoint):

```rust
use notify::{RecursiveMode, Watcher};
use tauri::{AppHandle, Manager};

use crate::broadcast;
use crate::commands::ingest::{build_offer, CardOffer};
use crate::state::IngestState;

/// Start the non-recursive `/Volumes` watcher on a background thread. Kept alive
/// for the process lifetime by moving the watcher into the spawned thread.
pub fn start_watcher(app: AppHandle) {
    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = match notify::recommended_watcher(tx) {
            Ok(w) => w,
            Err(_) => return,
        };
        if watcher.watch(Path::new("/Volumes"), RecursiveMode::NonRecursive).is_err() {
            return;
        }
        for res in rx {
            let Ok(event) = res else { continue };
            match event.kind {
                notify::EventKind::Create(_) => {
                    for path in event.paths {
                        handle_mount(&app, &path);
                    }
                }
                notify::EventKind::Remove(_) => {
                    for path in event.paths {
                        handle_unmount(&app, &path);
                    }
                }
                _ => {}
            }
        }
    });
}

/// Debounce a new mount until it exposes a card layout (up to ~1 s), then build
/// and publish the offer.
fn handle_mount(app: &AppHandle, mount: &Path) {
    for _ in 0..10 {
        let (tree, files) = walk_volume(mount);
        if is_card_ready(&tree) {
            if let Some(offer) = build_offer(mount, &tree, &files) {
                *app.state::<IngestState>().current.lock().expect("ingest lock") = Some(offer.clone());
                broadcast::card_detected(app, offer);
                let _ = crate::notify::notify(app, "Camera card ready", "Import clips into a project", "katto://ingest");
            }
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// Clear the offer if the unmounted volume was the detected card.
fn handle_unmount(app: &AppHandle, mount: &Path) {
    let mount_str = mount.to_string_lossy().into_owned();
    let mut guard = app.state::<IngestState>().current.lock().expect("ingest lock");
    if guard.as_ref().is_some_and(|o| o.volume == mount_str) {
        *guard = None;
        drop(guard);
        broadcast::card_removed(app);
    }
}

// `CardOffer` referenced above is re-exported here for the state module.
pub use crate::commands::ingest::CardOffer as _CardOffer;
```

> Note: `.expect(...)` on a `Mutex` lock is permitted outside `#[cfg(test)]` only if unavoidable; prefer `if let Ok(mut guard) = ...lock()` to satisfy the no-`expect` rule. Use this form instead:
> ```rust
> if let Ok(mut guard) = app.state::<IngestState>().current.lock() { *guard = Some(offer.clone()); }
> ```
> Apply the same `if let Ok` pattern in `handle_unmount`. (The plan's Step-5 snippet uses `.expect` for brevity; the implementer must convert to `if let Ok` before `just check`, since clippy `-D warnings` + the no-expect rule will reject it.)

- [ ] **Step 6: Wire the module** — add `mod volumes;` to `src-tauri/src/lib.rs`. (Watcher is started in Task 10 Step 6's `setup`.)

- [ ] **Step 7: Run the pure tests** — Run: `cargo test -p katto volumes`. Expected: 3 PASS.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/Cargo.toml Cargo.lock src-tauri/src/volumes.rs src-tauri/src/state.rs src-tauri/src/broadcast.rs src-tauri/src/lib.rs
git commit -m "feat(ingest): non-recursive /Volumes watcher with card debounce"
```

---

## Task 10: Ingest commands + wiring

**Files:**
- Create: `src-tauri/src/commands/ingest.rs`
- Modify: `src-tauri/src/commands.rs` (add `pub mod ingest;`)
- Modify: `src-tauri/src/notify.rs` (add `Route::Ingest`)
- Modify: `src-tauri/src/lib.rs` (register commands + events, `.manage(IngestState)`, start watcher, regenerate bindings)

**Interfaces:**
- Produces (wire DTOs, `Serialize + Deserialize + Clone + specta::Type`): `CardOffer { volume, kind, total_bytes, groups }`, `ClipGroupDto { label, clips }`, `ClipDto { path, name, size, is_video, selected, duration_s }`. Commands: `card_offer(ingest) -> Option<CardOffer>`; `start_ingest(state, ingest, app, volume, project_slug, selected_paths) -> Job`; `eject_card(volume) -> ()`; `import_files(state, app, project_slug, paths) -> Job`. Helper `build_offer(mount, tree, files) -> Option<CardOffer>`.

- [ ] **Step 1: Define the DTOs + `build_offer` (do this BEFORE Task 9)** — in `src-tauri/src/commands/ingest.rs`:

```rust
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use katto_engine::ingest::{enumerate::enumerate, recognize::recognize, FileEntry, VolumeTree};

use crate::db::jobs::Job;
use crate::error::{Error, Result};
use crate::state::{AppState, IngestState};

/// One clip in a card offer, as sent to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ClipDto {
    /// Source path relative to the volume root.
    pub path: String,
    /// File name.
    pub name: String,
    /// Byte size.
    pub size: u64,
    /// Whether it is a video (importable) vs a sidecar.
    pub is_video: bool,
    /// Default selection state.
    pub selected: bool,
    /// Duration in seconds, if ffprobe succeeded.
    pub duration_s: Option<f64>,
}

/// A group of clips sharing card substructure.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ClipGroupDto {
    /// Group label (substructure dir name).
    pub label: String,
    /// Clips in the group.
    pub clips: Vec<ClipDto>,
}

/// The current detected card, offered to the import sheet.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct CardOffer {
    /// Absolute mount path (`/Volumes/<NAME>`), also the eject target.
    pub volume: String,
    /// Recognized card kind, as a stable slug (`"sony"`/`"generic_dcim"`/`"iphone_dcim"`).
    pub kind: String,
    /// Total bytes of all video clips (for the free-space check).
    pub total_bytes: u64,
    /// Grouped clips.
    pub groups: Vec<ClipGroupDto>,
}

/// Build a `CardOffer` from a walked volume: recognize, enumerate, and attach
/// ffprobe durations (best-effort; a probe failure leaves `duration_s = None`).
pub fn build_offer(mount: &Path, tree: &VolumeTree, files: &[FileEntry]) -> Option<CardOffer> {
    let card = recognize(tree)?;
    let kind = match card.kind {
        katto_engine::ingest::CardKind::Sony => "sony",
        katto_engine::ingest::CardKind::GenericDcim => "generic_dcim",
        katto_engine::ingest::CardKind::IphoneDcim => "iphone_dcim",
    };
    let under_roots: Vec<FileEntry> = files
        .iter()
        .filter(|f| card.clip_roots.iter().any(|r| f.path.starts_with(r)))
        .cloned()
        .collect();
    let groups = enumerate(card.kind, &under_roots);

    let mut total_bytes = 0u64;
    let groups: Vec<ClipGroupDto> = groups
        .into_iter()
        .map(|g| ClipGroupDto {
            label: g.label,
            clips: g
                .clips
                .into_iter()
                .map(|c| {
                    if c.is_video {
                        total_bytes += c.size;
                    }
                    let duration_s = if c.is_video {
                        crate::ffprobe::probe_clip(&mount.join(&c.path)).ok().and_then(|m| m.duration_s)
                    } else {
                        None
                    };
                    ClipDto { path: c.path.to_string_lossy().into_owned(), name: c.name, size: c.size, is_video: c.is_video, selected: c.selected, duration_s }
                })
                .collect(),
        })
        .collect();

    Some(CardOffer { volume: mount.to_string_lossy().into_owned(), kind: kind.to_string(), total_bytes, groups })
}
```

- [ ] **Step 2: Add the commands** (same file, below `build_offer`):

```rust
/// The current detected card offer, if any.
#[tauri::command]
#[specta::specta]
pub async fn card_offer(ingest: State<'_, IngestState>) -> Result<Option<CardOffer>> {
    Ok(ingest.current.lock().map(|g| g.clone()).unwrap_or(None))
}

/// Validate the card mount, project, and free space, then spawn the copy job.
#[tauri::command]
#[specta::specta]
pub async fn start_ingest(
    state: State<'_, AppState>,
    app: AppHandle,
    volume: String,
    project_slug: String,
    selected_paths: Vec<String>,
) -> Result<Job> {
    spawn_ingest(&state, &app, PathBuf::from(&volume), project_slug, selected_paths).await
}

/// Manual drag-in path (iPhone footage): same rename+verify pipeline, absolute
/// source paths, no watcher/card involvement.
#[tauri::command]
#[specta::specta]
pub async fn import_files(
    state: State<'_, AppState>,
    app: AppHandle,
    project_slug: String,
    paths: Vec<String>,
) -> Result<Job> {
    // Sources are absolute; use the filesystem root ("/") as the source_root so
    // `Rename::source` carries the absolute path components.
    let sources: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    spawn_ingest_from_absolute(&state, &app, project_slug, sources).await
}

/// Eject the card by its mount path. `diskutil eject` accepts a mount point.
#[tauri::command]
#[specta::specta]
pub async fn eject_card(volume: String) -> Result<()> {
    let output = std::process::Command::new("diskutil").arg("eject").arg(&volume).output()?;
    if !output.status.success() {
        return Err(Error::Io(format!("eject failed: {}", String::from_utf8_lossy(&output.stderr))));
    }
    Ok(())
}
```

- [ ] **Step 3: Add the shared spawn helpers** (same file) — resolves the project footage dir, computes today's local date via SQLite, plans renames off the highest existing `NNN`, checks free space with `fs4`, and spawns `run_copy_job`:

```rust
use katto_engine::ingest::naming::plan_renames;

use crate::ingest::copy::{run_copy_job, CopyPlan};

/// Shared spawn path for both `start_ingest` (relative to a volume) and
/// `import_files` (absolute sources under `/`).
async fn spawn_ingest(
    state: &AppState,
    app: &AppHandle,
    volume: PathBuf,
    project_slug: String,
    selected_paths: Vec<String>,
) -> Result<Job> {
    let sources: Vec<PathBuf> = selected_paths.into_iter().map(PathBuf::from).collect();
    plan_and_spawn(state, app, volume, project_slug, sources).await
}

async fn spawn_ingest_from_absolute(state: &AppState, app: &AppHandle, project_slug: String, sources: Vec<PathBuf>) -> Result<Job> {
    // Strip the leading "/" so each source is relative to source_root = "/".
    let rels: Vec<PathBuf> = sources.iter().map(|p| p.strip_prefix("/").unwrap_or(p).to_path_buf()).collect();
    plan_and_spawn(state, app, PathBuf::from("/"), project_slug, rels).await
}

async fn plan_and_spawn(
    state: &AppState,
    app: &AppHandle,
    source_root: PathBuf,
    project_slug: String,
    sources: Vec<PathBuf>,
) -> Result<Job> {
    // Resolve footage dir, today's date, and the highest existing sequence in one db.call.
    let slug = project_slug.clone();
    let (footage_dir, existing, today): (PathBuf, Vec<String>, String) = state
        .db
        .call(move |conn| {
            let root = crate::commands::projects::require_mounted(conn)?;
            let project = crate::db::projects::get(conn, &slug)?
                .ok_or_else(|| Error::Db(format!("no such project: {slug}")))?;
            let footage = PathBuf::from(&root).join("Projects").join(&project.slug).join("footage");
            let existing: Vec<String> = std::fs::read_dir(&footage)
                .map(|rd| rd.flatten().filter_map(|e| e.file_name().into_string().ok()).collect())
                .unwrap_or_default();
            let date = project.shoot_date.clone().filter(|d| !d.is_empty()).unwrap_or_else(|| {
                conn.query_row("SELECT date('now','localtime')", [], |r| r.get::<_, String>(0)).unwrap_or_default()
            });
            Ok((footage, existing, date))
        })
        .await?;

    // Free-space guard on the studio root before any copy begins.
    let needed: u64 = sources
        .iter()
        .map(|s| std::fs::metadata(source_root.join(s)).map(|m| m.len()).unwrap_or(0))
        .sum();
    let free = fs4::available_space(&footage_dir).unwrap_or(0);
    if free < needed {
        return Err(Error::Io(format!("insufficient free space: need {needed} bytes, {free} free")));
    }

    // Plan renames: (path, lowercased ext) in stable order.
    let mut typed: Vec<(PathBuf, String)> = sources
        .into_iter()
        .map(|p| {
            let ext = p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).unwrap_or_default();
            (p, ext)
        })
        .collect();
    typed.sort_by(|a, b| a.0.cmp(&b.0));
    let renames = plan_renames(&today, &existing, &typed);

    std::fs::create_dir_all(&footage_dir)?;
    let plan = CopyPlan { source_root, footage_dir, renames, project_slug: project_slug.clone() };
    let db = state.db.clone();
    let app2 = app.clone();
    let label = format!("Import {} clips", plan.renames.len());
    state
        .jobs
        .spawn("ingest", &label, None, move |ctx| async move { run_copy_job(ctx, db, app2, plan).await })
        .await
}
```

> Verify `crate::commands::projects::require_mounted` and `crate::db::projects::get(conn, slug) -> Result<Option<Project>>` signatures against `src-tauri/src/commands/projects.rs` and `src-tauri/src/db/projects.rs` before implementing; adjust the `project.slug`/`project.shoot_date` field access to the real `Project` struct field names. If `require_mounted` is `pub(crate)` in `commands/projects.rs`, it is already reachable; otherwise widen its visibility in this task.

- [ ] **Step 4: Add `Route::Ingest`** — in `src-tauri/src/notify.rs`: add `Ingest` to the `Route` enum, `Route::Ingest => "ingest".to_string()` in `as_wire`, and in `parse_deep_link` add `if rest == "ingest" { return Some(Route::Ingest); }`. Add a test `parses_ingest_route` asserting `parse_deep_link("katto://ingest") == Some(Route::Ingest)`.

- [ ] **Step 5: Register the module** — add `pub mod ingest;` to `src-tauri/src/commands.rs`.

- [ ] **Step 6: Register commands, events, state, watcher** — in `src-tauri/src/lib.rs`:
  - add the four commands to `collect_commands![...]`: `commands::ingest::card_offer`, `commands::ingest::start_ingest`, `commands::ingest::eject_card`, `commands::ingest::import_files`;
  - add `broadcast::CardDetected`, `broadcast::CardRemoved` to `collect_events![...]`;
  - `.manage(crate::state::IngestState::default())` alongside the existing `.manage(...)`;
  - in the `setup` hook, after the app handle exists, call `crate::volumes::start_watcher(app.handle().clone());`.

- [ ] **Step 7: Regenerate the bindings** — Run: `cargo test -p katto export_bindings`. Expected: PASS; `src/lib/ipc/bindings.gen.ts` now contains `cardOffer`, `startIngest`, `ejectCard`, `importFiles`, `CardOffer`, `ClipGroupDto`, `ClipDto`, and the two events. (The hook protects `bindings.gen.ts` from hand edits — only this generator may touch it.)

- [ ] **Step 8: Build + backend gate** — Run: `cargo test -p katto` then `cargo clippy -p katto -- -D warnings`. Expected: clean (convert any `.expect` on locks to `if let Ok`).

- [ ] **Step 9: Run the Rust reviewer** — dispatch the `rust-reviewer` agent on the backend diff (Tasks 1–10) before moving to the frontend; fix confirmed findings.

- [ ] **Step 10: Commit**

```bash
git add src-tauri/src/commands/ingest.rs src-tauri/src/commands.rs src-tauri/src/notify.rs src-tauri/src/lib.rs src/lib/ipc/bindings.gen.ts
git commit -m "feat(ingest): card_offer/start_ingest/eject/import commands and wiring"
```

---

## Task 11: Frontend IPC wrapper + UI primitives

**Files:**
- Create: `src/lib/ipc/ingest.ts`
- Create: `src/components/ui/checkbox.tsx`, `src/components/ui/callout.tsx`

**Interfaces:**
- Produces: `ingestKeys`; `cardOffer()`, `startIngest(volume, projectSlug, selectedPaths)`, `ejectCard(volume)`, `importFiles(projectSlug, paths)`; re-exported `CardOffer`, `ClipGroupDto`, `ClipDto` types; `subscribeJobProgress` re-used from `@/lib/ipc/jobs`.

- [ ] **Step 1: Write the IPC wrapper** — `src/lib/ipc/ingest.ts`:

```ts
import { commands } from "@/lib/ipc/bindings.gen";
import type { CardOffer, ClipDto, ClipGroupDto } from "@/lib/ipc/bindings.gen";
import { unwrap } from "@/lib/ipc/result";

export type { CardOffer, ClipDto, ClipGroupDto };

export const ingestKeys = {
	all: ["ingest"] as const,
	offer: () => [...ingestKeys.all, "offer"] as const,
};

export const cardOffer = () => unwrap(commands.cardOffer());

export const startIngest = (volume: string, projectSlug: string, selectedPaths: string[]) =>
	unwrap(commands.startIngest(volume, projectSlug, selectedPaths));

export const ejectCard = (volume: string) => unwrap(commands.ejectCard(volume));

export const importFiles = (projectSlug: string, paths: string[]) =>
	unwrap(commands.importFiles(projectSlug, paths));
```

- [ ] **Step 2: Add the primitives** — try the shadcn CLI first: `bunx shadcn@latest add checkbox`. If it errors under the CSS-first Tailwind v4 setup, hand-write both. `src/components/ui/checkbox.tsx` (radix, ember-tinted, 16px):

```tsx
import { Checkbox as CheckboxPrimitive } from "radix-ui";
import { Check } from "@phosphor-icons/react";
import type * as React from "react";

import { cn } from "@/lib/utils";

function Checkbox({ className, ...props }: React.ComponentProps<typeof CheckboxPrimitive.Root>) {
	return (
		<CheckboxPrimitive.Root
			data-slot="checkbox"
			className={cn(
				"size-4 shrink-0 rounded-[var(--r)] border border-border bg-surface",
				"data-[state=checked]:border-ember data-[state=checked]:bg-ember data-[state=checked]:text-on-ember",
				"focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ember focus-visible:ring-offset-2",
				"disabled:opacity-45",
				className,
			)}
			{...props}
		>
			<CheckboxPrimitive.Indicator className="flex items-center justify-center">
				<Check size={12} weight="bold" />
			</CheckboxPrimitive.Indicator>
		</CheckboxPrimitive.Root>
	);
}

export { Checkbox };
```

`src/components/ui/callout.tsx` (the free-space warning surface — opts out of grain per the translucent-fill rule):

```tsx
import type * as React from "react";

import { cn } from "@/lib/utils";

function Callout({ className, children, ...props }: React.ComponentProps<"div">) {
	return (
		<div
			data-slot="callout"
			style={{ backgroundImage: "none" }}
			className={cn(
				"flex items-start gap-2 rounded-[var(--r)] border border-warn/40 bg-warn/10 px-3 py-2 text-sm text-fg",
				className,
			)}
			{...props}
		>
			{children}
		</div>
	);
}

export { Callout };
```

- [ ] **Step 3: Typecheck** — Run: `bunx tsc --noEmit`. Expected: no errors (confirms `bindings.gen.ts` exports the ingest types from Task 10).

- [ ] **Step 4: Commit**

```bash
git add src/lib/ipc/ingest.ts src/components/ui/checkbox.tsx src/components/ui/callout.tsx
git commit -m "feat(ingest): typed ingest ipc wrapper and checkbox/callout primitives"
```

---

## Task 12: Ingest model (pure selection / size / default-project / free-space math)

**Files:**
- Create: `src/features/ingest/model/select.ts`
- Create: `src/features/ingest/model/select.test.ts`

**Interfaces:**
- Produces: `formatBytes(n: number): string`; `formatDuration(s: number | null): string`; `defaultProjectSlug(projects, today): string | null`; `selectedClips(offer, selected): ClipDto[]`; `selectionTotals(offer, selected): { count: number; bytes: number }`; `hasEnoughFreeSpace(bytes, freeBytes): boolean`; `allPathsIn(group): string[]`.

- [ ] **Step 1: Write the failing test** — `src/features/ingest/model/select.test.ts`:

```ts
import { describe, expect, it } from "vitest";

import type { CardOffer } from "@/lib/ipc/ingest";
import {
	allPathsIn,
	defaultProjectSlug,
	formatBytes,
	formatDuration,
	selectionTotals,
} from "@/features/ingest/model/select";

const offer: CardOffer = {
	volume: "/Volumes/SONY",
	kind: "sony",
	total_bytes: 300,
	groups: [
		{
			label: "CLIP",
			clips: [
				{ path: "CLIP/C0001.MP4", name: "C0001.MP4", size: 100, is_video: true, selected: true, duration_s: 12.5 },
				{ path: "CLIP/C0001.XML", name: "C0001.XML", size: 5, is_video: false, selected: false, duration_s: null },
				{ path: "CLIP/C0002.MP4", name: "C0002.MP4", size: 200, is_video: true, selected: true, duration_s: null },
			],
		},
	],
};

describe("formatBytes", () => {
	it("renders GB with one decimal", () => {
		expect(formatBytes(22 * 1024 ** 3)).toBe("22.0 GB");
	});
	it("renders MB below a gigabyte", () => {
		expect(formatBytes(500 * 1024 ** 2)).toBe("500 MB");
	});
});

describe("formatDuration", () => {
	it("formats mm:ss", () => {
		expect(formatDuration(72)).toBe("1:12");
	});
	it("shows a dash when unknown", () => {
		expect(formatDuration(null)).toBe("—");
	});
});

describe("selectionTotals", () => {
	it("counts only selected paths and sums their bytes", () => {
		const selected = new Set(["CLIP/C0001.MP4", "CLIP/C0002.MP4"]);
		expect(selectionTotals(offer, selected)).toEqual({ count: 2, bytes: 300 });
	});
});

describe("defaultProjectSlug", () => {
	it("picks the project whose shoot_date is nearest today", () => {
		const projects = [
			{ slug: "far", shoot_date: "2026-07-01" },
			{ slug: "near", shoot_date: "2026-07-21" },
		];
		expect(defaultProjectSlug(projects, "2026-07-22")).toBe("near");
	});
	it("returns null with no projects", () => {
		expect(defaultProjectSlug([], "2026-07-22")).toBeNull();
	});
});

describe("allPathsIn", () => {
	it("returns only video clip paths for select-all", () => {
		expect(allPathsIn(offer.groups[0]!)).toEqual(["CLIP/C0001.MP4", "CLIP/C0002.MP4"]);
	});
});
```

- [ ] **Step 2: Run it red** — Run: `bunx vitest run src/features/ingest/model/select.test.ts`. Expected: FAIL (module missing).

- [ ] **Step 3: Implement** — `src/features/ingest/model/select.ts`:

```ts
import type { CardOffer, ClipDto, ClipGroupDto } from "@/lib/ipc/ingest";

/** Human-readable byte size: GB with one decimal at/above 1 GiB, else whole MB. */
export function formatBytes(n: number): string {
	const gib = 1024 ** 3;
	if (n >= gib) return `${(n / gib).toFixed(1)} GB`;
	return `${Math.round(n / 1024 ** 2)} MB`;
}

/** Duration as `m:ss`, or an em dash when unknown. */
export function formatDuration(s: number | null): string {
	if (s === null) return "—";
	const total = Math.round(s);
	const mins = Math.floor(total / 60);
	const secs = total % 60;
	return `${mins}:${secs.toString().padStart(2, "0")}`;
}

/** Every video clip path in a group (used for the per-group select-all). */
export function allPathsIn(group: ClipGroupDto): string[] {
	return group.clips.filter((c) => c.is_video).map((c) => c.path);
}

/** The selected clips across all groups. */
export function selectedClips(offer: CardOffer, selected: ReadonlySet<string>): ClipDto[] {
	return offer.groups.flatMap((g) => g.clips.filter((c) => selected.has(c.path)));
}

/** Count and total bytes of the current selection. */
export function selectionTotals(offer: CardOffer, selected: ReadonlySet<string>): { count: number; bytes: number } {
	const clips = selectedClips(offer, selected);
	return { count: clips.length, bytes: clips.reduce((sum, c) => sum + c.size, 0) };
}

/** True when free space covers the selection. */
export function hasEnoughFreeSpace(bytes: number, freeBytes: number): boolean {
	return freeBytes >= bytes;
}

interface ProjectLike {
	slug: string;
	shoot_date: string | null;
}

/** The project whose `shoot_date` is nearest `today` (ISO `YYYY-MM-DD`), or null. */
export function defaultProjectSlug(projects: readonly ProjectLike[], today: string): string | null {
	const dated = projects.filter((p): p is ProjectLike & { shoot_date: string } => !!p.shoot_date);
	if (dated.length === 0) return projects[0]?.slug ?? null;
	const todayMs = Date.parse(today);
	let best = dated[0]!;
	let bestDelta = Math.abs(Date.parse(best.shoot_date) - todayMs);
	for (const p of dated.slice(1)) {
		const delta = Math.abs(Date.parse(p.shoot_date) - todayMs);
		if (delta < bestDelta) {
			best = p;
			bestDelta = delta;
		}
	}
	return best.slug;
}
```

- [ ] **Step 4: Run it green** — Run: `bunx vitest run src/features/ingest/model/select.test.ts`. Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add src/features/ingest/model/select.ts src/features/ingest/model/select.test.ts
git commit -m "feat(ingest): pure selection, size, and default-project model"
```

---

## Task 13: Import sheet + grouped clip list

**Files:**
- Create: `src/features/ingest/store/ingest-sheet.ts`
- Create: `src/features/ingest/hooks/use-card-offer.ts`
- Create: `src/features/ingest/components/clip-group-list.tsx`
- Create: `src/features/ingest/components/import-sheet.tsx`
- Create: `src/features/ingest/components/import-sheet.test.tsx`

**Interfaces:**
- Consumes: `cardOffer`, `startIngest`, `ingestKeys`, the model fns, `projectsKeys`/`getProjects` (verify the list wrapper name in `src/lib/ipc/projects.ts`), `createProject`.
- Produces: `useIngestSheetStore`; `useCardOffer()`; `<ImportSheet />`; `<ClipGroupList />`.

- [ ] **Step 1: Sheet store** — `src/features/ingest/store/ingest-sheet.ts`:

```ts
import { create } from "zustand";

interface IngestSheetState {
	open: boolean;
	setOpen: (open: boolean) => void;
}

export const useIngestSheetStore = create<IngestSheetState>((set) => ({
	open: false,
	setOpen: (open) => set({ open }),
}));
```

- [ ] **Step 2: Card-offer hook** — `src/features/ingest/hooks/use-card-offer.ts` (query + live event refresh; opens the sheet on detect):

```ts
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import { events } from "@/lib/ipc/bindings.gen";
import { cardOffer, ingestKeys } from "@/lib/ipc/ingest";
import { useIngestSheetStore } from "@/features/ingest/store/ingest-sheet";

export function useCardOffer() {
	const queryClient = useQueryClient();
	const setOpen = useIngestSheetStore((s) => s.setOpen);

	useEffect(() => {
		const detected = events.cardDetected.listen(() => {
			queryClient.invalidateQueries({ queryKey: ingestKeys.offer() });
			setOpen(true);
		});
		const removed = events.cardRemoved.listen(() => {
			queryClient.invalidateQueries({ queryKey: ingestKeys.offer() });
		});
		return () => {
			void detected.then((un) => un());
			void removed.then((un) => un());
		};
	}, [queryClient, setOpen]);

	return useQuery({ queryKey: ingestKeys.offer(), queryFn: cardOffer });
}
```

> Verify `events.cardDetected` / `events.cardRemoved` are the generated names in `bindings.gen.ts` (tauri-specta lowercases the event struct to camelCase). Adjust if the generator emits a different accessor.

- [ ] **Step 3: Grouped clip list** — `src/features/ingest/components/clip-group-list.tsx`:

```tsx
import { FilmSlate } from "@phosphor-icons/react";

import type { CardOffer } from "@/lib/ipc/ingest";
import { Checkbox } from "@/components/ui/checkbox";
import { allPathsIn, formatBytes, formatDuration } from "@/features/ingest/model/select";

interface Props {
	offer: CardOffer;
	selected: ReadonlySet<string>;
	onToggle: (path: string, on: boolean) => void;
	onToggleGroup: (paths: string[], on: boolean) => void;
}

export function ClipGroupList({ offer, selected, onToggle, onToggleGroup }: Props) {
	return (
		<div className="flex flex-col gap-4">
			{offer.groups.map((group) => {
				const paths = allPathsIn(group);
				const allOn = paths.length > 0 && paths.every((p) => selected.has(p));
				return (
					<div key={group.label} className="flex flex-col gap-1">
						<div className="flex items-center gap-2 px-1 py-1 text-fg-muted">
							<Checkbox checked={allOn} onCheckedChange={(v) => onToggleGroup(paths, v === true)} />
							<span className="text-sm">{group.label}</span>
							<span className="ml-auto font-mono text-xs tabular-nums">{paths.length} clips</span>
						</div>
						<ul className="flex flex-col">
							{group.clips.filter((c) => c.is_video).map((clip) => (
								<li key={clip.path} className="flex items-center gap-2 rounded-[var(--r)] px-1 py-1.5 hover:bg-surface-2">
									<Checkbox checked={selected.has(clip.path)} onCheckedChange={(v) => onToggle(clip.path, v === true)} />
									<FilmSlate size={16} className="text-fg-faint" />
									<span className="truncate text-sm">{clip.name}</span>
									<span className="ml-auto font-mono text-xs text-fg-muted tabular-nums">{formatDuration(clip.duration_s)}</span>
									<span className="w-20 text-right font-mono text-xs text-fg-muted tabular-nums">{formatBytes(clip.size)}</span>
								</li>
							))}
						</ul>
					</div>
				);
			})}
		</div>
	);
}
```

- [ ] **Step 4: Import sheet** — `src/features/ingest/components/import-sheet.tsx`. Composes `Dialog`, a project `Select` (default via `defaultProjectSlug`), `ClipGroupList`, the size/free-space summary, and the Import button (`Import to <project>`). Full component:

```tsx
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { Warning } from "@phosphor-icons/react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Callout } from "@/components/ui/callout";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { getProjects, projectsKeys } from "@/lib/ipc/projects";
import { startIngest } from "@/lib/ipc/ingest";
import { ClipGroupList } from "@/features/ingest/components/clip-group-list";
import { useCardOffer } from "@/features/ingest/hooks/use-card-offer";
import { defaultProjectSlug, formatBytes, selectionTotals } from "@/features/ingest/model/select";
import { useIngestSheetStore } from "@/features/ingest/store/ingest-sheet";

export function ImportSheet() {
	const open = useIngestSheetStore((s) => s.open);
	const setOpen = useIngestSheetStore((s) => s.setOpen);
	const { data: offer } = useCardOffer();
	const { data: projects = [] } = useQuery({ queryKey: projectsKeys.all, queryFn: getProjects });
	const queryClient = useQueryClient();

	const today = new Date().toISOString().slice(0, 10);
	const [projectSlug, setProjectSlug] = useState<string | null>(null);
	const activeSlug = projectSlug ?? defaultProjectSlug(projects, today);

	const [selected, setSelected] = useState<ReadonlySet<string>>(() => new Set());
	// Initialize the selection from the offer's default-selected video clips.
	useMemo(() => {
		if (offer) {
			const initial = new Set(offer.groups.flatMap((g) => g.clips.filter((c) => c.selected).map((c) => c.path)));
			setSelected(initial);
		}
	}, [offer]);

	const totals = offer ? selectionTotals(offer, selected) : { count: 0, bytes: 0 };
	const activeProject = projects.find((p) => p.slug === activeSlug);

	const importMutation = useMutation({
		mutationFn: () => {
			if (!offer || !activeSlug) throw new Error("no card or project");
			return startIngest(offer.volume, activeSlug, [...selected]);
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["jobs"] });
			setOpen(false);
			toast.success(`Importing ${totals.count} clips`);
		},
	});

	if (!offer) return null;

	const toggle = (path: string, on: boolean) =>
		setSelected((prev) => {
			const next = new Set(prev);
			if (on) next.add(path);
			else next.delete(path);
			return next;
		});
	const toggleGroup = (paths: string[], on: boolean) =>
		setSelected((prev) => {
			const next = new Set(prev);
			for (const p of paths) on ? next.add(p) : next.delete(p);
			return next;
		});

	return (
		<Dialog open={open} onOpenChange={setOpen}>
			<DialogContent className="max-w-lg">
				<DialogHeader>
					<DialogTitle>Import {totals.count} clips</DialogTitle>
				</DialogHeader>

				<div className="flex flex-col gap-4">
					<Select value={activeSlug ?? undefined} onValueChange={setProjectSlug}>
						<SelectTrigger>
							<SelectValue placeholder="Choose a project" />
						</SelectTrigger>
						<SelectContent>
							{projects.map((p) => (
								<SelectItem key={p.slug} value={p.slug}>{p.title}</SelectItem>
							))}
						</SelectContent>
					</Select>

					<div className="max-h-72 overflow-y-auto">
						<ClipGroupList offer={offer} selected={selected} onToggle={toggle} onToggleGroup={toggleGroup} />
					</div>

					<div className="flex items-center justify-between text-sm text-fg-muted">
						<span>{totals.count} selected</span>
						<span className="font-mono tabular-nums">{formatBytes(totals.bytes)}</span>
					</div>

					{!activeProject?.mounted && activeSlug && (
						<Callout>
							<Warning size={16} className="mt-0.5 text-warn" />
							<span>The studio drive is disconnected. Reconnect it before importing.</span>
						</Callout>
					)}

					<div className="flex justify-end gap-2">
						<Button variant="secondary" onClick={() => setOpen(false)}>Cancel</Button>
						<Button onClick={() => importMutation.mutate()} disabled={totals.count === 0 || !activeSlug || importMutation.isPending}>
							Import to {activeProject?.title ?? "project"}
						</Button>
					</div>
				</div>
			</DialogContent>
		</Dialog>
	);
}
```

> Verify against `src/lib/ipc/projects.ts`: the list wrapper name (`getProjects` vs `listProjects`), the `Project` fields (`slug`, `title`, `shoot_date`, `mounted`), and the `Select`/`Dialog` sub-component export names in `src/components/ui/`. Adjust imports/field names to the real API. The free-space Callout above is a mount guard; the byte-level free-space refusal is enforced server-side in `start_ingest` (Task 10) and surfaced as a toast on the mutation error — do not duplicate the numeric check client-side beyond the display total.

- [ ] **Step 5: Behavior test** — `src/features/ingest/components/import-sheet.test.tsx`, mocking IPC with `mockIPC` (assert: renders the group list, the title reflects selection count, clicking Import calls `start_ingest` with the selected paths). Follow the `mockIPC`/`clearMocks` pattern in `src/test/setup.ts` and an existing feature test. Run: `bunx vitest run src/features/ingest/components/import-sheet.test.tsx`. Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/features/ingest/store/ingest-sheet.ts src/features/ingest/hooks/use-card-offer.ts src/features/ingest/components/clip-group-list.tsx src/features/ingest/components/import-sheet.tsx src/features/ingest/components/import-sheet.test.tsx
git commit -m "feat(ingest): import sheet with grouped clip list and project select"
```

---

## Task 14: Progress panel + success/eject state

**Files:**
- Create: `src/features/ingest/components/ingest-progress.tsx`

**Interfaces:**
- Consumes: `subscribeJobProgress` from `@/lib/ipc/jobs`, `ejectCard` from `@/lib/ipc/ingest`, `Progress`, `Button`.
- Produces: `<IngestProgress jobId volume projectTitle clipCount />` — the macOS copy-sheet-styled panel that streams progress and, on completion, offers Eject.

- [ ] **Step 1: Implement** — `src/features/ingest/components/ingest-progress.tsx` (reuses the exact `subscribeJobProgress` pattern from `src/features/dashboard/active-jobs.tsx`):

```tsx
import { useMutation } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { CheckCircle, Eject } from "@phosphor-icons/react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import type { JobProgress } from "@/lib/ipc/bindings.gen";
import { subscribeJobProgress } from "@/lib/ipc/jobs";
import { ejectCard } from "@/lib/ipc/ingest";

interface Props {
	jobId: string;
	volume: string;
	projectTitle: string;
	clipCount: number;
}

export function IngestProgress({ jobId, volume, projectTitle, clipCount }: Props) {
	const [progress, setProgress] = useState(0);
	const [message, setMessage] = useState<string | null>(null);
	const [done, setDone] = useState(false);

	useEffect(() => {
		let cancelled = false;
		void subscribeJobProgress(jobId, (update: JobProgress) => {
			if (cancelled) return;
			setProgress(update.progress);
			setMessage(update.message);
			if (update.progress >= 1) setDone(true);
		});
		return () => {
			cancelled = true;
		};
	}, [jobId]);

	const eject = useMutation({
		mutationFn: () => ejectCard(volume),
		onSuccess: () => toast.success("Card ejected — safe to remove"),
	});

	return (
		<div className="flex flex-col gap-2 rounded-[var(--r-lg)] border border-border bg-surface p-4">
			<div className="flex items-center gap-2">
				{done ? <CheckCircle size={16} className="text-done" weight="fill" /> : null}
				<span className="text-sm">
					{done ? `Imported ${clipCount} clips → ${projectTitle}` : `Copying ${clipCount} clips → ${projectTitle}`}
				</span>
			</div>
			<Progress value={progress * 100} />
			{message && !done ? <span className="font-mono text-xs text-fg-muted tabular-nums">{message}</span> : null}
			{done ? (
				<div className="flex justify-end">
					<Button variant="secondary" onClick={() => eject.mutate()} disabled={eject.isPending}>
						<Eject size={16} /> Eject card
					</Button>
				</div>
			) : null}
		</div>
	);
}
```

> The failure path: when the copy job fails, the terminal `JobProgress` carries the error `message` and the job row is `failed` — the dashboard's active-jobs list already surfaces failed jobs (Phase 1). Keep this panel's job in the shared jobs list so a failed ingest shows there too; no extra failure UI is required here beyond leaving the last message visible.

- [ ] **Step 2: Typecheck** — Run: `bunx tsc --noEmit`. Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add src/features/ingest/components/ingest-progress.tsx
git commit -m "feat(ingest): copy-progress panel with eject on completion"
```

---

## Task 15: Deep-link route, global sheet mount, and manual drag-in

Wires the sheet into the app, routes `katto://ingest`, and adds the project-detail footage card whose drop target runs `import_files`. **This task edits `project-detail.tsx`, which carries uncommitted DateInput hunks — commit is stash-gated (see steps).**

**Files:**
- Modify: `src/app/` composition root (mount `<ImportSheet />` once) — find the top-level app component (e.g. `src/app/app.tsx`).
- Modify: `src/hooks/use-deep-link-router.ts` (handle the `"ingest"` route).
- Modify: `src/features/projects/detail/project-detail.tsx` (add the footage/drag-in card).

**Interfaces:**
- Consumes: `getCurrentWebview().onDragDropEvent` from `@tauri-apps/api/webview`, `importFiles`, `useIngestSheetStore`.

- [ ] **Step 1: Mount the sheet globally** — in the app composition root, render `<ImportSheet />` once at the top level (it returns `null` until a card is offered). Import from `@/features/ingest/components/import-sheet`.

- [ ] **Step 2: Route the deep link** — in `src/hooks/use-deep-link-router.ts`, add a branch: when the broadcast route string is `"ingest"`, call `useIngestSheetStore.getState().setOpen(true)` (and navigate to the dashboard/ingest surface as the router does for other routes). Follow the existing switch pattern in that file.

- [ ] **Step 3: Add the footage drag-in card** — in `project-detail.tsx`, add a `FootageCard` alongside `FreshnessCard` that listens for file drops and calls `importFiles(slug, paths)`. Sketch (adapt to the file's card idiom):

```tsx
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useEffect, useState } from "react";
import { FilmStrip } from "@phosphor-icons/react";
import { toast } from "sonner";

import { Card } from "@/components/ui/card";
import { importFiles } from "@/lib/ipc/ingest";

function FootageCard({ slug }: { slug: string }) {
	const [over, setOver] = useState(false);
	useEffect(() => {
		let cancelled = false;
		const unlisten = getCurrentWebview().onDragDropEvent((event) => {
			if (cancelled) return;
			if (event.payload.type === "over") setOver(true);
			else if (event.payload.type === "leave") setOver(false);
			else if (event.payload.type === "drop") {
				setOver(false);
				void importFiles(slug, event.payload.paths).then(() =>
					toast.success(`Importing ${event.payload.paths.length} files`),
				);
			}
		});
		return () => {
			cancelled = true;
			void unlisten.then((un) => un());
		};
	}, [slug]);

	return (
		<Card className={over ? "border-ember" : undefined}>
			<div className="flex items-center gap-2 text-fg-muted">
				<FilmStrip size={20} />
				<span className="text-sm">Drop iPhone footage here to import into this project</span>
			</div>
		</Card>
	);
}
```

Render `<FootageCard slug={slug} />` in the detail layout.

> The `onDragDropEvent` `drop` payload carries absolute `paths` (verified against `@tauri-apps/api` v2) — HTML drag-drop would not. This is why the manual path uses the Tauri webview event, not a DOM `ondrop`. The whole-window listener fires for any drop; scope the import to when the pointer is over the footage card if the file's layout makes stray drops likely (compare `event.payload.position` against the card's bounding rect) — otherwise the single card on the detail page is an acceptable target.

- [ ] **Step 4: Typecheck** — Run: `bunx tsc --noEmit`. Expected: clean.

- [ ] **Step 5: Run the frontend reviewer** — dispatch `frontend-reviewer` on the `src/` diff (Tasks 11–15); fix confirmed findings.

- [ ] **Step 6: Stash-gated commit** — the DateInput hunks in `project-detail.tsx` / `project-detail.test.tsx` / `main.css` must NOT be committed. Sequence:

```bash
# Stash ONLY the pre-existing DateInput working changes (their hunks coexist in project-detail.tsx).
git stash push -- src/features/projects/detail/project-detail.tsx src/features/projects/detail/project-detail.test.tsx src/styles/main.css
```

If that stash also removes this task's FootageCard additions (same file), instead stage this task's other files first and handle `project-detail.tsx` with an interactive-free patch: commit the app-root + router changes separately, then add only the ingest hunks of `project-detail.tsx` via `git add -p` is disallowed (no interactive) — so the robust path is:

```bash
# 1) Commit the non-conflicting files first.
git add src/app/ src/hooks/use-deep-link-router.ts
git commit -m "feat(ingest): mount import sheet globally and route katto://ingest"
# 2) For project-detail.tsx (mixed DateInput + FootageCard), verify with git diff that
#    only intended ingest hunks are staged. Since git add is whole-file, confirm the
#    DateInput hunks are already committed or belong to the owner's uncommitted set:
git status --short src/features/projects/detail/project-detail.tsx
```

**Decision for the implementer:** because `git add` is whole-file and interactive `add -p` is unavailable, and the owner's DateInput hunks live in the same file, do NOT stage `project-detail.tsx` in this run. Instead, place the `FootageCard` in a **new sibling file** `src/features/projects/detail/footage-card.tsx` and change `project-detail.tsx` only if unavoidable. If a one-line render addition to `project-detail.tsx` is required, leave it uncommitted alongside the owner's DateInput hunks and note it in `docs/overnight-run.md` as owner-reviewable — never `git add` that file. Commit the standalone card:

```bash
git add src/features/projects/detail/footage-card.tsx
git commit -m "feat(ingest): footage drag-in card for manual iphone import"
```

> This keeps the hard constraint intact: `project-detail.tsx`, `project-detail.test.tsx`, and `main.css` are never staged in this phase. The manual drag-in ships as a standalone `footage-card.tsx`; wiring it into the detail view (a single JSX line) is deferred to the owner or done as an uncommitted change, recorded in the overnight-run checklist.

---

## Task 16: Owner checklist + full gate

**Files:**
- Modify: `docs/overnight-run.md` (untracked — append checkboxes)

- [ ] **Step 1: Append owner-testable items** — add to `docs/overnight-run.md`:

```markdown
## Phase 3 — SD Ingest (owner visual + hardware verification)

- [ ] Insert the real Lexar V90 card from the ZV-E10 II → tray pulses + a "Camera card ready" notification appears.
- [ ] Click the notification (or open `katto://ingest`) → the import sheet opens with clips grouped by CLIP/SUB, videos pre-selected, sidecars (.XML/.THM) hidden/deselected.
- [ ] The project selector defaults to the project whose shoot date is nearest today; "Import to <project>" reflects the choice.
- [ ] Click Import → the copy-progress panel shows "Copying N clips → <project>" with a live bar; the tray mirrors the percentage.
- [ ] On completion → footage lands in `Projects/<slug>/footage/` renamed `YYYY-MM-DD_NNN.ext`, sequence continuing from any existing files for that date; the card is byte-for-byte unchanged (spot-check a file size).
- [ ] "Eject card" button appears → clicking it ejects the card (Finder shows it gone); a second insert re-detects cleanly.
- [ ] Free-space guard: with a nearly-full studio drive, Import refuses with an exact-numbers toast and copies nothing.
- [ ] Failure path: yank the card mid-copy → the job goes `failed`, an `<name>.partial` is left quarantined, completed files remain, the card is untouched.
- [ ] iPhone path: build the `footage-card.tsx` into the project detail (one JSX line, currently uncommitted) → drag a `.mov` from the iPhone folder onto it → same rename+verify pipeline, no watcher involvement.
- [ ] `events` log shows an `ingested {count, bytes, project}` row after a successful import.
```

- [ ] **Step 2: Run the full gate** — Run: `just check` from the workspace root. Expected: fmt-check + clippy `-D warnings` + cargo test + tsc all green. Paste the tail when reporting. Fix anything red before declaring the phase done.

- [ ] **Step 3: Commit** — `docs/overnight-run.md` is untracked and stays that way per CLAUDE.md; do NOT `git add` it. (It is the owner's local checklist.) If any tracked doc (e.g. `prd/index.md` status row) is updated to mark Phase 3 landed, commit that separately:

```bash
# Only if the status tracker was updated:
git add prd/index.md
git commit -m "docs(prd): mark phase 3 sd-ingest landed"
```

---

## Self-review

**Spec coverage (phase-3 PRD scope table → task):**
- Volume watcher (non-recursive `notify` on `/Volumes`, debounce, unmount clears) → Task 9. (metadata via `diskutil info -plist` is intentionally *not* implemented — recognition is folder-marker based, eject uses the mount path, free space uses `fs4`; noted as a scope simplification.)
- Card recognition (`DCIM/` generic+iPhone, Sony `PRIVATE/M4ROOT/CLIP/`, clip roots + kind, non-camera ignored) → Task 3.
- Clip enumeration (video exts case-insensitive, name/size, duration+codec via ffprobe, grouped by substructure, sidecars deselected) → Tasks 4 (grouping/classification) + 7 (ffprobe) + 10 (`build_offer` attaches duration).
- Import sheet (one question = project, default nearest shoot_date, grouped list all selected, size total + free-space) → Tasks 12–13. Inline "New project" → uses existing `createProject`; wired via the same Select's project list (owner can create beforehand; a minimal inline-create affordance can be added in Task 13's Select footer — flagged for the implementer).
- Copy job (copy-only, source read-only, `footage/`, `YYYY-MM-DD_NNN.ext`, sequence continuation, collisions impossible) → Tasks 5 + 8 + 10.
- Verification (count + per-file size, mismatch → failed + `.partial` quarantine, card untouched) → Tasks 6 + 8.
- Progress + events (`kind='ingest'` job, per-file + overall over `Channel`, tray mirror, `ingested` events row) → Tasks 8 + 10 + 14.
- Eject (`diskutil eject`, failure reported not retried) → Tasks 10 + 14.
- iPhone/manual path (same pipeline, no watcher) → Tasks 10 (`import_files`) + 15 (`footage-card.tsx`).
- Error handling (free space refusal with numbers, mid-copy failure, card yank, ffprobe failure non-blocking, studio root unmounted) → Task 10 (free-space + `require_mounted` → `StudioRootUnmounted`), Task 8 (mid-copy/yank → `.partial`), Task 7/10 (ffprobe failure leaves `duration_s = None`).
- Testing (pure recognition/grouping over fixture trees, rename/sequence, verify comparator; tempdir copy with injected failure; manual hardware) → Tasks 3–6 (pure), 8 (tempdir), 16 (hardware checklist).

**Inline-new-project gap:** the PRD's "New project inline (title → create_project)" is only partially specified above (the sheet lists existing projects). The implementer should add an inline "New project…" item to the project `Select` in Task 13 that opens a one-field title prompt calling `createProject(title, null)` then selects the new slug — this reuses the Phase-2 command and needs no backend work. Flagged rather than fully coded because the exact `createProject` signature and any Phase-2 new-project dialog to reuse must be read from `src/lib/ipc/projects.ts` / `src/features/projects/` first.

**Type consistency:** engine types (`CardKind`, `VolumeTree`, `Card`, `FileEntry`, `ClipEntry`, `ClipGroup`, `Rename`, `VerifyError`, `MediaInfo`) are defined once in Task 1–6 and consumed by name in Tasks 7–10. Wire DTOs (`CardOffer`, `ClipGroupDto`, `ClipDto`) are defined once in Task 10 Step 1 and consumed in Tasks 9, 11, 12, 13. `run_copy_job`/`CopyPlan`/`copy_one` names are consistent Tasks 8↔10. `subscribeJobProgress`, `startIngest`, `ejectCard`, `importFiles`, `cardOffer` names are consistent across `ingest.ts` and its consumers.

**Known verification points the implementer must confirm against the real code (flagged inline at each site):** `crate::commands::projects::require_mounted` visibility/signature; `crate::db::projects::get` return + `Project` field names (`slug`/`title`/`shoot_date`/`mounted`); the projects list IPC wrapper name (`getProjects` vs `listProjects`); the generated event accessors (`events.cardDetected` casing); the `Select`/`Dialog` sub-component export names; and whether `bunx shadcn add checkbox` works under Tailwind v4 CSS-first (hand-written fallback provided). None of these change the plan's structure — they are last-mile name confirmations per the repo's "verify before you build" rule.
