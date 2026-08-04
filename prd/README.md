# katto → Studio OS — PRD

**Status:** approved. This folder is the in-repo source of truth for building Studio OS.
Start at [index.md](index.md) for the doc map and phase status.

## Vision

katto grows from a transcript-driven rough-cut editor into the owner's personal **Studio OS**:
a menu-bar-resident macOS application that runs his entire YouTube production workflow end to
end — planning what to shoot, ingesting footage, AI-planning the rough cut, exporting editable
NLE timelines, gathering assets, scaffolding thumbnails, and orchestrating every AI task
through a visible Claude session dock.

The organizing thesis: creators who grow have a **connected workflow where each stage feeds
the next**. katto is that spine. It is choreography, never intelligence — all judgment lives
in Claude sessions and the owner's existing skills; katto schedules, launches, files, and
displays.

Reference products: Kiru (transcript-cut UX), Docker Desktop (menu-bar residency),
Raycast/Linear (⌘K palette, UI joy), Hedge/Kyno (ingest patterns), Notion (the "OS" feel).

## Architecture

```
┌──────────────────────────── katto.app (menu-bar resident) ───────────────────────────┐
│ React/TS UI (Vite, Tailwind v4, shadcn-style Radix copy-in, cmdk, TanStack Query +   │
│   Zustand (+zundo), tauri-specta typed IPC, xterm.js, wavesurfer v7)                 │
│   Dashboard · Planner (Board/Calendar/Backlog) · Projects · Project detail           │
│   Cut Editor (3-pane) · Claude Dock panel · Browser tabs · Settings/Onboarding       │
├───────────────────────────────── Tauri 2 IPC ────────────────────────────────────────┤
│ Rust core (src-tauri): tray + live state · jobs framework · scheduler (catch-up) ·   │
│   session pool (PTY) · volume/file watchers · download interception · SQLite (WAL) · │
│   keychain · spawns: ffmpeg/ffprobe · claude · open -a · diskutil eject              │
├──────────────────────────────────────────────────────────────────────────────────────┤
│ crates/katto-engine (pure lib): Rational math · schema+validators · import ·         │
│   transcribe (ElevenLabs) · CutPlanner · cuts↔edits merge · FCPXML 1.11 emitter      │
│   (+rescue track) · MP4 render · SRT/VTT                                             │
│ crates/katto-cli: thin wrapper (cut/import/transcribe/plan/render/export/auth)       │
└──────────────────────────────────────────────────────────────────────────────────────┘
External brains: Claude Code sessions (+ hyper-frames skills) · ElevenLabs · FCP/Resolve
Data: app-support SQLite (index only) · Studio root, external SSD recommended (folders = truth)
```

**Normative IPC rules:** JS owns live edit state (bundle loaded in one call; edits are pure
JS mutations; the debounced 200 ms auto-save is the only interactive bridge call); long ops
stream via `Channel<T>`; media bytes never cross `invoke` (asset protocol / `convertFileSrc`);
rational time end-to-end in the engine.

## Decisions log (locked)

Approved section-by-section by the owner (2026-07-02 design session). Do not re-litigate.

| # | Decision | Rationale |
|---|----------|-----------|
| D1 | **Personal, macOS-only tool.** Supersedes the original spec's Windows/Linux targets and public-product framing. | One user, one Mac; engineering optimizes for that. Repo may stay public. |
| D2 | **Stack: Tauri 2 + Rust core + React/TS/Vite UI + local SQLite (WAL), menu-bar resident** (tray icon, click-to-open, sleep-to-tray, launch-at-login — Docker Desktop pattern). | Electron too heavy for a tray citizen; native Swift too slow for UI iteration given the owner's React fluency; custom "joyous" UI is faster in web tech. |
| D3 | **Vertical-slice build order:** every phase builds UI + backend and wires them before moving on. | No "engine first, shell later" dead ends; each phase ships something usable. |
| D4 | **Existing assets are reused, not discarded:** `crates/katto-engine`, `crates/katto-cli`, the cut-decider prompt, the clean-audio pipeline, still-normative original-spec sections. | The rough-cut core already has a validated design and reference implementation. |
| D5 | **Project = one video = a folder first, DB row second.** Folder is truth; SQLite only indexes (rescan/reconcile on launch; a folder deleted in Finder disappears from the app). Projects live under a **Studio root** chosen at onboarding — an external SSD (recommended) or a local folder, boot-volume choices warned but never blocked; an unmounted removable root shows an explicit "drive disconnected" state. | If katto died, every project remains a clean, self-explanatory folder. |
| D6 | **Folder anatomy:** `project.json`, `footage/` (**flat** — no a-cam/b-cam split; `YYYY-MM-DD_NNN.ext` naming), `audio/`, `assets/{envato,vfx,graphics,music,sfx}`, `thumbnails/`, `timelines/` (versioned `-vN`, never overwritten), `exports/`. `assets/vfx/` is engine-agnostic (HyperFrames, Remotion, AE — one folder per effect: project file + render). The `.kruproj` cut bundle lives **inside** the owning project folder; loose-bundle open stays supported. | Owner decision on flatness; engine-agnostic VFX keeps the NLE referencing plain rendered files. The music/sfx library sits under `assets/` (already the home for sourced third-party material) rather than under `audio/`, which is pipeline working space holding the `.kruproj` cut bundles; both folders are filled by hand. |
| D7 | **Data model:** `ideas`, `projects`, `schedule`, `events` (+ `jobs`, `scheduled_jobs` support tables). `ideas`/`raw_signal` keep column parity with hyper-frames `tools/studio` so its discovery CLI and curation pattern work unchanged. **No numeric scoring anywhere; AI suggests, the human decides** ("aggregator, never a judge"). | Carried from the owner's prior tool; parity makes the one-time import and shared tooling trivial. |
| D8 | **Planner = Board / Calendar / Backlog**; global quick-capture hotkey (idea → backlog from anywhere); promoting an idea creates the project folder + card in one motion. `tools/studio` stays untouched and parallel until katto's planner proves out; **one-time idea import from its `studio.db`** at cutover. | Keep the working tool alive until the replacement earns trust. |
| D9 | **Scheduled curation:** anacron-style scheduler with catch-up semantics ("daily at 00:00; if the Mac slept through it and last success > N h, run once now — never pile up"). First job: nightly idea curation via a visible Claude session (studio-ideas pattern: qualitative keep/discard + one-line rationale, never a grade) → macOS notification → deep-link to Ideas. | Laptops sleep; cron doesn't forgive that. Judgment stays in Claude, visibly. |
| D10 | **SD ingest is dumb and deterministic — no AI in this path.** Copy-only (card never modified), rename to `YYYY-MM-DD_NNN.ext`, post-copy count/size verification, progress in window + tray, eject offer, events row. **Descoped by owner decision:** checksummed offload, simultaneous second-destination backup, hover-scrub thumbnails, AirDrop watcher. iPhone footage lands in the same flat folder manually. | Reliability through simplicity; Hedge-grade features are overkill for one user. |
| D11 | **Cut editor = original spec M1–M8 with deltas:** macOS-only; **rescue-track export** (kept segments on the FCPXML spine + a second disabled/muted track carrying every removed segment — a wrong AI cut costs one click in the NLE, not a re-run; Recut pattern); timelines versioned, never overwritten; "Open in Final Cut" after export. | The transcript-primary editor design was already validated; the deltas de-risk AI cuts. |
| D12 | **Primary NLE: Final Cut Pro. Export format: FCPXML 1.11 only** (opens natively in FCP, imports into Resolve and Premiere). A Premiere-dialect XML only if FCPXML proves insufficient. Later enhancement: drive **Resolve Studio's Python scripting API** to build the project directly. Never assume free Resolve (it can't hardware-decode the camera's 10-bit 4:2:2 H.265). | Hardware research: FCP fully uses the M5 Pro Media Engine; one format imports everywhere. |
| D13 | **ffmpeg's role shrinks** to cheap audio extraction (and the optional MP4 render); the slow concat render stops being the default deliverable. | The NLE timeline is the product; rendering is optional. |
| D14 | **BYOK AI runtime:** subprocess `claude` default (the user's install holds subscription auth; we never touch the OAuth token), Anthropic API key fallback, keys in the macOS keychain. Single-shot prompt + transcript → cuts JSON; no agent-SDK loop. Cut planning initially runs via the subprocess planner; once the Claude Dock exists it re-routes through a visible dock session. | Anthropic ToS prohibits third-party OAuth subscription use; subprocess sidesteps it cleanly. |
| D15 | **Claude Dock:** every AI task routes through a visible session-pool (real Claude Code sessions over Rust PTYs, xterm.js panel; the owner can type, interrupt, redirect — app pushes and keyboard share the same seat). Icon states idle/running/needs-input/done; one tab per task; pool scales on demand; idle reaping (default 5 min; options 2/10) with the interactive panel exempt; failed tasks leave their tab open with the error. **VFX cockpit:** dock sessions open in `assets/vfx/<effect>/` with the HyperFrames/Remotion toolchain; folder watch surfaces renders. | "Nothing runs invisibly" is the trust contract for AI doing real work. |
| D16 | **Project browser:** minimal real browser in-app (tab strip, search-or-navigate address bar — non-URL input goes to Google, multi-tab via Tauri multi-webview, persistent logged-in session, no preloaded home site: the pane and every new tab open on a start page with a search field and fixed site tiles, so a tab may hold no URL). One special power: **any download from any tab is intercepted and filed into the active project's `assets/`** (unzipped if archive, sidecar JSON with item URL + license timestamp). Interception failure → `~/Downloads` + notice. Explicitly not a Chrome clone: no extensions, no profiles. | Asset gathering is the workflow step that leaks time; filing must be automatic. |
| D17 | **Thumbnails stay deliberately small:** scaffold `thumbnails/<slug>-thumb-a.psd` from bundled templates (1280×720 + 1080×1920, guide layers + safe zones), open Photoshop, folder-watch the exported PNG onto the project card. Thumbnail *intelligence* stays in the owner's skills, invocable via the dock. | katto is choreography; design judgment is not its job. |
| D18 | **Error philosophy: nothing fails silently.** Every background op is a job with visible state (tray + events log). Unmounted-drive guard; failed Claude task keeps its dock tab + error; download fallback; timelines schema-validated before write; missed scheduled runs execute once on wake. | A single-user tool dies by silent breakage, not by missing features. |
| D19 | **Testing matched to risk:** golden-file snapshots on emitters (byte-identical FCPXML/SRT), pure-function tests for ingest renaming and scheduler catch-up, light component tests, one-time manual FCP/Resolve import checks, events log doubling as field diagnostics. | Test effort goes where corruption or data loss lives. |
| D21 | **macOS residency: `ActivationPolicy::Regular` (Dock icon + app menu), not `Accessory`.** Supersedes the "no Dock icon (`ActivationPolicy::Accessory`)" clause of the Phase-1 tray-residency criterion. Real Docker Desktop — the D2 reference — shows a Dock icon and app menu, so Regular is the faithful reading of "Docker Desktop pattern," not a departure from it. Close-to-tray keeps idle RAM minimal by **destroying the WebView on window close and recreating it on demand** (tray item / Dock `Reopen` / second launch), with the process kept alive via `RunEvent::ExitRequested` → `prevent_exit` (only when `code` is `None`, so tray Quit still exits). Left-click on the tray opens its menu; the window is driven by a dynamic Show/Hide item. Native quit paths (⌘Q, app-menu Quit, Dock-menu Quit) stay functional — only teardown-initiated `ExitRequested` is prevented, via an explicit flag, since `code` alone cannot distinguish them. **Implementation requirement once editing surfaces exist (Phase 4+):** because close destroys the WebView, live UI edit state must be flushed to disk on `beforeunload` before teardown — the debounced auto-save is not guaranteed to have fired at close time. | katto has no always-on engine like Docker's VM — the WebView is the only heavy part, so freeing it on close (rather than hiding) is what makes "resident in the menu bar" cost ~15 MB instead of ~200 MB. Owner decision, 2026-07-05. |
| D20 | **Onboarding:** Studio root picker (external SSD recommended; a boot-volume or low-free-space choice warns but is never blocked — footage is large), ElevenLabs key (keychain), `claude` PATH detection. The `default_nle` setting is **not** asked at onboarding — it is seeded lazily at the first export (the export dialog forces a pick when none is stored) and thereafter pre-selected as a sticky last-used default, changeable in the dialog or on the Settings page. **Packaging:** `.dmg` via Tauri bundler, launch-at-login; signing/notarization deferred until distribution matters (Apple Developer ID, $99/yr). | First-run must establish the three things katto cannot guess; the NLE target is a post-export choice (D12), not a first-run fact. |

**Hardware context the app serves:** MacBook Pro 14" M5 Pro (Media Engine hardware-decodes
H.265 10-bit 4:2:2 — the camera's exact codec) · Sony ZV-E10 II (4K60, XAVC HS/S, UHS-II SD,
`PRIVATE/M4ROOT/` card layout) · iPhone 14 Pro B-cam (Blackmagic Camera app) · Acasis TBU405
Pro Thunderbolt NVMe enclosure = the external Studio root · ElevenLabs (Scribe v2) and Envato
Elements subscriptions · a mature hyper-frames skills library (script-writer, thumbnails,
shorts, publish QA, studio-ideas, audio-cut-decider).

## Supersedes

The original cut-editor spec (`app_design_rough_cut.md`, local-only — see below) remains
normative for the cut-editor core **except** where this PRD's decisions override it: its §11
cross-platform (Windows/Linux) targets and public-product framing are superseded by D1; its
§7 serde-struct FCPXML outline is superseded by the quick-xml `Writer` decision in
[phase-5.md](phase-5.md). Its still-normative sections (IPC/state ownership, bundle format,
AI runtime, editor UX, export details, error handling, testing strategy) are distilled into
phases 4–5 here.

## Source availability

Some source material is **gitignored and local-only** (private): `docs/superpowers/`
(both design specs), root `agents/cut-decider.md` (the cut-planner system prompt), and root
`skills/clean-audio/` (the TS reference pipeline + Zod validators + fixtures). A fresh clone
does not have them. The PRDs compensate:

- Every decision and contract needed to build is **stated in this folder** — no PRD requires
  reading the gitignored specs.
- Where a phase needs a local-only file *verbatim* (the cut-decider prompt body, the
  clean-audio validators/fixtures), its PRD marks it as an **external input** and states the
  full contract so the phase is implementable either by porting the original from the owner's
  machine or from the contract alone. See [phase-4.md](phase-4.md) and [phase-6.md](phase-6.md).
- The `hyper-frames/` mirror **is** committed and is the normative reference for everything it
  covers (studio DB schema, promote flow, cut-video ffmpeg math, audio-editor UX patterns).
