# katto — TODO

Source of truth: `docs/superpowers/specs/app_design_rough_cut.md`.

Order is dependency-driven: each milestone unblocks the next. Do not start a milestone until the previous one's "done when" is true.

---

## M0 — Workspace restructure (start here)

The current repo is a single-crate Tauri scaffold. The spec wants a Cargo workspace with the engine as a library, a CLI binary, and the Tauri app as a third member sharing the engine.

- [ ] Convert root `Cargo.toml` into a workspace manifest with members: `crates/katto-engine`, `crates/katto-cli`, `src-tauri`
- [ ] Create `crates/katto-engine` (library crate, no UI deps)
- [ ] Create `crates/katto-cli` (binary crate, depends on `katto-engine`)
- [ ] Update `src-tauri/Cargo.toml` to depend on `katto-engine` via workspace path
- [ ] Verify `cargo build --workspace` succeeds and the existing Tauri dev shell still launches (`bun run tauri dev`)
- [ ] Pin a Rust toolchain (`rust-toolchain.toml`, stable)

**Done when:** workspace builds clean; Tauri app still runs the default scaffold; `cargo run -p katto-cli -- --help` prints clap-generated help.

---

## M1 — Project bundle + import

Lay down the data shapes everything else will read and write. Get bytes on disk before any network calls.

- [ ] Define serde types for `project.json`, `cuts.json`, `edits.json` in `katto-engine` (mirror schemas in `agents/cut-decider.md` for cuts)
- [ ] Introduce the `Rational { num: i64, den: u32 }` type and use it for every time-bearing field (spec §7 invariant)
- [ ] `engine::import(video_path) -> Project`: extract mono 16kHz audio with `ffmpeg`, probe frame rate via `ffprobe -show_streams -select_streams v` reading `r_frame_rate`, write the `.kruproj/` skeleton
- [ ] Bundle open/save round-trip with snapshot test (`open → mutate → save → open → equal`)
- [ ] CLI: `katto import <video>` wired to `engine::import`

**Done when:** `katto import sample.mp4` produces a valid bundle on disk; round-trip test passes.

---

## M2 — Transcription (ElevenLabs Scribe v2)

- [ ] OS keychain wrapper (Keychain / Credential Manager / libsecret) — read/write the ElevenLabs key
- [ ] `engine::transcribe(project)` — POSTs `cached_audio.wav` to ElevenLabs Scribe v2, stores raw response in `transcript.json`
- [ ] Retry / error surfacing per spec §8 (no partial writes)
- [ ] CLI: `katto transcribe <project>` and `katto auth status`
- [ ] Integration test with a recorded fixture response (`--features expensive-tests` for the real-network variant)

**Done when:** running `katto transcribe` on an imported project produces a valid `transcript.json`; offline test against fixture passes.

---

## M3 — Cut planner

- [ ] Define the `CutPlanner` trait in `katto-engine` per spec §2
- [ ] `SubprocessClaudePlanner`: spawn `claude --print --output-format json`, pipe transcript to stdin, parse cuts JSON from stdout
- [ ] `HttpAnthropicPlanner`: POST to `/v1/messages` with `cut-decider.md` as system prompt; default model `claude-sonnet-4-6`
- [ ] First-run detection (PATH probe of `claude --version`)
- [ ] Schema validation for `cuts.json` (overlap, ordering, token alignment, total duration); single retry then surface
- [ ] CLI: `katto plan <project>` with `--mode subprocess|http` override

**Done when:** both planners produce a schema-valid `cuts.json` for a fixture transcript; invariant violations trigger one retry then a clear error.

---

## M4 — Export (FCPXML, MP4, captions)

This is the export trifecta — get all three working together so v1 has something to show.

- [ ] FCPXML 1.11 emitter using `quick-xml` + serde derive structs; field order frozen with snapshot tests
- [ ] All time fields emit as `<num>/<den>s` rationals; drop-frame TC handled for 29.97/59.94
- [ ] `<asset>` references the source via `url::Url::from_file_path` (percent-encoded `file:///`)
- [ ] MP4 render via `ffmpeg -filter_complex_script @file` (write filtergraph to temp file to dodge argv limits); fallback to segment-and-concat for very long edit lists; fixture test with 500+ cuts
- [ ] SRT/VTT re-timestamping per §7 (drop in-cut, subtract prior cut total, group by sentence/42 chars)
- [ ] CLI: `katto render <project>`, `katto export <project>`
- [ ] Manual smoke: import generated `.fcpxml` into Final Cut Pro and DaVinci Resolve; clip count + durations + frame alignment match

**Done when:** end-to-end CLI pipeline works (`katto cut <video>` → MP4 + FCPXML + SRT) on a real clip and FCP/Resolve open the FCPXML cleanly.

---

## M5 — App: load + view

First time the app does something the CLI doesn't. Read-only at this stage.

- [ ] Tauri command: `open_bundle(path) -> { project, transcript, cuts, edits }` returning the full payload (one call, JS owns state from then on per §2)
- [ ] Tauri `Channel<T>` plumbing for streaming progress (`import`, `transcribe`, `plan`)
- [ ] Frontend: React + TS + Vite + Tailwind + Radix scaffolding
- [ ] Three-pane layout: transcript (left), `<video>` (center), timeline (bottom)
- [ ] Transcript pane renders token-aligned spans with cut/discretionary/flag styling (read-only)
- [ ] Click-word-to-seek using `video.currentTime = t` (pure DOM, no Rust round-trip)
- [ ] Serve `cached_audio.wav` and frame thumbs via `convertFileSrc` / `asset:` protocol
- [ ] WaveSurfer.js waveform on the timeline pane
- [ ] Canvas timeline track with thumbnails (every 2s) and cut/discretionary overlays

**Done when:** opening a bundle in the app shows the transcript, plays the video, and clicking words seeks correctly.

---

## M6 — App: edit

- [ ] Toggle a cut (click struck word → uncut; click discretionary → apply)
- [ ] Drag-select span + `X`/Delete → manual cut, snapped to token boundaries
- [ ] Timeline drag-edge boundary adjustment, snap-to-token by default, Option/Alt for free-drag
- [ ] Kept-only playback mode via `timeupdate` listener seeking past cut ranges; toggle for show-original
- [ ] Transport keys: Space, J/K/L, arrows (frame ±1), Shift+arrow (±10)
- [ ] Undo/redo (Cmd/Ctrl+Z, Shift+Z) with one edit per step
- [ ] Auto-save: debounce 200ms, write `edits.json`, persist undo log so it survives reopen

**Done when:** a user can take an AI-planned bundle, refine cuts in the app, close and reopen, and pick up where they left off.

---

## M7 — App: AI runtime + first-run

- [ ] Settings panel: shows Claude Code detection state, lets user override Mode A → Mode B, manages API keys via keychain
- [ ] First-run flow: probe `claude`, prompt for ElevenLabs key, optionally prompt for Anthropic key
- [ ] Wire `engine::transcribe` and `engine::plan_cuts` into the app with streaming progress (per-stage indicators per §4)
- [ ] Incremental cut rendering as Claude streams (parse JSON as it arrives so cuts appear in the transcript live)

**Done when:** a user can drag a video onto the app, watch transcribe + plan run with progress, and edit the result.

---

## M8 — Polish + ship

- [ ] Source-video relocation dialog (filename + duration match) per §8
- [ ] All error surfaces wired through Tauri events (no blocking dialogs)
- [ ] Cross-platform smoke: macOS, Windows, Linux
- [ ] Distribution: Tauri bundles (.dmg / .msi / .AppImage); Homebrew tap + GitHub Releases for engine/CLI
- [ ] Resolve open items in spec §11 (final name, license split, default model, telemetry stance)

**Done when:** signed/notarized builds for all three OSes; install → import → edit → export works on each.

---

## Cross-cutting checklist (apply throughout)

- Rational time end-to-end; only convert to float at UI edges (§7)
- No media bytes through `invoke` — `convertFileSrc` for audio/video/thumbs (§2)
- Long-running ops stream via `Channel<T>`, never poll
- Keychain for every secret; never log key material
- Snapshot tests on FCPXML field order (FCP rejects out-of-order elements)
- Real-network tests behind `--features expensive-tests`
