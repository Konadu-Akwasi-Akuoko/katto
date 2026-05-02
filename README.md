# katto

Transcript-driven rough-cut editor for long-form video. Transcribe, AI-plan cuts, refine, export to FCP / DaVinci Resolve / Premiere.

> **Status:** early development. Working name `katto` (placeholder, may change). Not yet released.

## What it does

`katto` turns raw long-form footage into a usable rough cut:

1. **Transcribe** the audio (ElevenLabs Scribe v2)
2. **Plan cuts** with an LLM — fillers, retakes, false starts, long silences
3. **Refine** the cuts in a transcript-primary editor
4. **Export** to FCPXML 1.11 (FCP / Resolve / Premiere), MP4, and SRT/VTT captions

It does the boring 80% of cutting and hands a clean starting point to your real NLE.

## Why

Most video editors spend hours scrubbing through long-form footage to remove ums, retakes, and dead air before the actual creative edit can begin. `katto` automates that first pass and gives you back a project file you can open in the tool you already use.

## Architecture

Two artifacts in one repo:

- **`katto-engine`** — pure-Rust library + CLI. Owns the pipeline: import, transcribe, plan, render, export.
- **`katto-app`** — Tauri 2 desktop app. React/TS frontend, Rust backend wrapping the engine.

A detailed design document is maintained alongside the code; it will be published in `docs/` once it stabilizes.

## Getting started

> Build instructions will land once the workspace split is in place. For now this repo contains a fresh Tauri scaffold.

### Prerequisites

- Rust (stable)
- [Bun](https://bun.sh/)
- `ffmpeg` and `ffprobe` on `PATH`
- An ElevenLabs API key
- Either [Claude Code](https://docs.claude.com/en/docs/claude-code) installed (preferred) or an Anthropic API key

### Run the app (dev)

```bash
bun install
bun run tauri dev
```

## Roadmap

- [ ] Workspace split: `katto-engine` (lib + CLI) + `katto-app` (Tauri)
- [ ] `import`: ffmpeg audio extraction + project bundle skeleton
- [ ] `transcribe`: ElevenLabs Scribe v2 integration
- [ ] `plan`: cut-decider via subprocess Claude Code or direct Anthropic HTTP
- [ ] `render`: ffmpeg concat MP4 export
- [ ] `export`: FCPXML 1.11 + SRT/VTT
- [ ] Editor UI: transcript pane + video preview + timeline

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for setup, code style, and the PR process. By participating, you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).

## License

[MIT](LICENSE) © 2026 Konadu Akwasi Akuoko
