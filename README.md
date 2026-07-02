<div align="center">

# katto — Studio OS

**A personal, macOS-only, menu-bar-resident Studio OS for one YouTube channel.**

Plan → ingest → AI rough cut → NLE export → assets → thumbnails, with every AI task
visible in a Claude session dock.

[![CI](https://github.com/Konadu-Akwasi-Akuoko/katto/actions/workflows/ci.yml/badge.svg)](https://github.com/Konadu-Akwasi-Akuoko/katto/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-2.x-24C8DB?logo=tauri&logoColor=white)](https://v2.tauri.app/)
[![React](https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=white)](https://react.dev/)
[![TypeScript](https://img.shields.io/badge/TypeScript-strict-3178C6?logo=typescript&logoColor=white)](https://www.typescriptlang.org/)

</div>

> **Status:** in active development, built phase by phase from the PRDs in [`prd/`](prd/).
> This is a **personal tool for one user on one Mac** — the repo is public so the code can be
> read and reused, but it is not a product: no feature requests, no cross-platform support,
> and breaking changes whenever the owner's workflow wants them. Working name `katto`
> (placeholder, may change).

## What it is

katto runs a single creator's entire YouTube production workflow end to end from the macOS
menu bar:

1. **Plan** — idea backlog (global quick-capture hotkey), Kanban board, shoot/publish calendar;
   nightly AI curation that suggests, never scores
2. **Ingest** — SD card in, verified renamed footage in the right project folder, card ejected
3. **Rough cut** — transcribe (ElevenLabs Scribe v2) → AI cut plan (Claude) → refine in a
   transcript-primary editor
4. **Export** — FCPXML 1.11 with a rescue track (every removed segment on a muted second
   track), versioned into the project; opens straight in Final Cut Pro
5. **Assets & thumbnails** — an in-app browser that files downloads into the project
   automatically; PSD scaffolding with a folder watch
6. **The Claude Dock** — every AI task runs as a real, visible, interruptible Claude Code
   session in a terminal panel

Projects are **folders first, database rows second**: everything lives under a Studio root on
an external SSD, and katto only indexes it. If katto died tomorrow, every project would remain
a clean, self-explanatory folder.

## Source of truth

| Where | What |
|---|---|
| [`prd/README.md`](prd/README.md) | Vision, architecture, the locked decisions log |
| [`prd/index.md`](prd/index.md) | Doc map, feature→phase matrix, status tracker |
| `prd/phase-1.md` … `prd/phase-7.md` | One complete PRD per build phase |
| [`TODO.md`](TODO.md) | Phase checklist |
| `hyper-frames/` | Read-only mirror of the owner's skills/agents/tools repo — normative reference where katto overlaps it |

## Workspace

| Member | Role |
|---|---|
| `crates/katto-engine` | Pure-Rust library: rational time, cut-plan validation, import/transcribe/plan pipeline, FCPXML/MP4/SRT emitters. No UI deps. |
| `crates/katto-cli` | Thin CLI over the engine (`katto cut / import / transcribe / plan / render / export / auth status`) |
| `src-tauri` | The Tauri 2 app: tray, jobs, scheduler, session pool, watchers, SQLite — a thin IPC shell over the engine |
| `src/` | React 19 + TypeScript + Vite frontend (bun) |

## Development

Prerequisites: Rust (stable, pinned via `rust-toolchain.toml`), [Bun](https://bun.sh/),
[`just`](https://github.com/casey/just), `ffmpeg`/`ffprobe` on PATH, and for AI features
either [Claude Code](https://docs.claude.com/en/docs/claude-code) or an Anthropic API key,
plus an ElevenLabs key.

```bash
bun install
bun run tauri dev   # the app
just check          # the full quality gate (CI mirrors this 1:1)
```

Engineering conventions live in [`CLAUDE.md`](CLAUDE.md) and `.claude/rules/` — the repo is
built largely by AI agents working from the PRDs under those rules.

## Acknowledgements

- [Kiru](https://kiru.app) — transcript-driven editing UX reference
- [Descript](https://www.descript.com/) — the category's original
- [Tauri](https://v2.tauri.app/), [ElevenLabs](https://elevenlabs.io/),
  [Anthropic](https://www.anthropic.com/) — the load-bearing external pieces

## License

[MIT](LICENSE) © 2026 [Konadu Akwasi Akuoko](https://github.com/Konadu-Akwasi-Akuoko)
