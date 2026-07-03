# katto — Studio OS

Personal, macOS-only, menu-bar-resident Tauri 2 app running the owner's entire YouTube
production workflow: planning → SD ingest → AI rough cut → NLE export → assets → thumbnails,
with every AI task visible in a Claude session dock. One user, one Mac. Not a public product.

## Source of truth

- **`prd/` is the in-repo source of truth.** Start at `prd/index.md` (doc map, feature→phase
  matrix, status). Each `prd/phase-N.md` is a complete PRD — implement a phase from its PRD
  without re-deriving decisions. Locked decisions + rationale: `prd/README.md`.
- The original design specs live in `docs/superpowers/` which is **gitignored (local-only,
  private)** — never reference it as if committed; the PRDs distill everything needed from it.
  Same for root `agents/` (cut-decider prompt) and `skills/` (clean-audio reference pipeline):
  local-only inputs, ported into the repo when their phase implements them.
- `hyper-frames/` is a read-only reference mirror of the owner's skills/agents/tools repo.
  Where a katto module overlaps it (studio DB schema, promote flow, cut-video ffmpeg math,
  audio-editor UX, curation judgment), read the mirror first and reuse its schemas verbatim.

## Workspace

- `crates/katto-engine` — pure Rust library (media pipeline, validators, emitters). Never
  depends on tauri or UI concerns.
- `crates/katto-cli` — thin CLI over the engine.
- `src-tauri` — the Tauri app crate (`katto`, lib `katto_lib`); thin IPC shell over the engine.
- `src/` — React 19 + TypeScript + Vite frontend. Package manager: **bun** (never npm/yarn).

Dependency direction is one-way: app → engine, cli → engine.

## Commands

- **Gate: `just check`** (fmt-check + clippy `-D warnings` + cargo test + tsc). CI mirrors it
  1:1. Never claim work done without it passing. Run from the workspace root.
- Dev app: `bun run tauri dev`. Frontend only: `bun run dev`. **The owner runs the dev server
  personally most of the time — don't start it yourself; assume it's already running or ask.**

## Invariants (non-negotiable)

- Rational time (`Rational {num, den}`) end-to-end in the engine; floats only at UI and
  model/transcript boundaries.
- Media bytes never cross `invoke` — asset protocol only. JS owns live edit state; debounced
  auto-save is the only interactive bridge call; long ops stream via `Channel<T>`.
- Folders are truth; SQLite is an index reconciled on launch.
- Nothing fails silently: every background op is a `jobs` row + `events` row.
- No numeric scoring/ranking anywhere in planning or curation — AI suggests, human decides.
- Versioned exports (`timelines/*-vN`) are never overwritten; artifact writes are atomic
  (`.tmp` → rename).
- Desktop shell: the window never scrolls. `src/styles/main.css` locks `body`
  (`overflow: hidden`, `overscroll-behavior: none`, `user-select: none`); `AppShell`
  (`src/components/layout/app-shell.tsx`) is a fixed `100dvh` grid — titlebar + sidebar are
  pinned and its single content pane (`[data-scroll-root]`, `overflow-y-auto min-h-0`) is the
  only scroll region. Nest further scrollers inside it, never restore document scroll. The
  residual macOS WKWebView elastic bounce is killed natively (`scrollView.bounces = false` via
  `objc2`) in Phase 2.
- Conventional commits, one concern per commit, tests travel with their feature commit.

## Guidance layout

- Per-language style rules auto-load from `.claude/rules/` (rust, tauri-commands, frontend,
  testing, design-system) — follow them; don't restate them here. The visual language
  (`design-system`) has its full spec in `docs/superpowers/specs/2026-07-03-katto-design-system.md`.
- Procedures: `add-tauri-command`, `add-db-migration`, `add-feature-surface`,
  `emitter-snapshot-change` skills. Invoke before doing those tasks by hand.
- Hooks enforce formatting, protect generated files (`src-tauri/gen/`, `bindings.gen.ts`,
  lockfiles, `.snap.new`), and gate turn-end on a fast check. Reviewer agents `rust-reviewer`
  and `frontend-reviewer` run on the diff before a task is declared done.
